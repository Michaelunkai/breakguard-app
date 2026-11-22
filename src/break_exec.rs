//! Break-start action executor (box 9): runs the chosen [`BreakAction`]s when a
//! break starts and restores what it changed when the break (or the action's own
//! duration) ends. Every step is logged to breakguard.log with prefix `action`.
#![allow(dead_code)]

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use windows_sys::Win32::System::Shutdown::LockWorkStation;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::break_action::{actions_from_text, ActionKind, BreakAction};
use crate::engine::log_line;
use crate::sys;

/// What a started action must undo later.
#[derive(Clone, Debug, PartialEq)]
enum Undo {
    DdcOn(String),
    Overlay(String),
    AllMonitorsOn,
    Unmute,
    ResumeMedia,
    RestoreWindows,
}

fn pending() -> &'static Mutex<Vec<(u64, Undo)>> {
    static P: OnceLock<Mutex<Vec<(u64, Undo)>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(Vec::new()))
}

fn gen() -> &'static Mutex<u64> {
    static G: OnceLock<Mutex<u64>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(0))
}

/// Actions configured in settings.ini (`break_actions=`), default `[None]`.
pub fn configured() -> Vec<BreakAction> {
    let text = std::fs::read_to_string(crate::engine::Settings::path()).unwrap_or_default();
    let v = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("break_actions=").map(str::to_string))
        .unwrap_or_default();
    actions_from_text(&v)
}

/// Run every action for a break of `break_secs`. Non-blocking: work runs on a thread.
/// Actions with `duration_secs > 0` (shorter than the break) restore themselves early.
pub fn start(actions: Vec<BreakAction>, break_secs: u32) {
    let id = {
        let mut g = gen().lock().unwrap_or_else(|e| e.into_inner());
        *g += 1;
        *g
    };
    std::thread::spawn(move || {
        for a in actions {
            let undo = run_one(&a, break_secs);
            if undo.is_empty() {
                continue;
            }
            let d = a.duration_secs;
            if d > 0 && d < break_secs {
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(u64::from(d)));
                    for u in undo {
                        if take_one(id, &u) {
                            log_line(&format!("action restore after own duration {d} s:"));
                            restore(&u);
                        }
                    }
                });
            } else {
                let mut p = pending().lock().unwrap_or_else(|e| e.into_inner());
                p.extend(undo.into_iter().map(|u| (id, u)));
            }
        }
    });
}

fn take_one(id: u64, u: &Undo) -> bool {
    let mut p = pending().lock().unwrap_or_else(|e| e.into_inner());
    // early-restore entries are not in `pending`; they belong to the break `id` only
    let still = *gen().lock().unwrap_or_else(|e| e.into_inner()) == id;
    p.retain(|(i, x)| !(*i == id && x == u));
    still
}

/// Break over: undo everything still active. Blocking for at most a few DDC calls.
pub fn end() {
    let list: Vec<Undo> = std::mem::take(&mut *pending().lock().unwrap_or_else(|e| e.into_inner()))
        .into_iter()
        .map(|x| x.1)
        .collect();
    *gen().lock().unwrap_or_else(|e| e.into_inner()) += 1;
    if list.is_empty() {
        log_line("action restore at break end: nothing to restore");
    }
    for u in list.iter().rev() {
        restore(u);
    }
}

fn run_one(a: &BreakAction, break_secs: u32) -> Vec<Undo> {
    let secs = if a.duration_secs > 0 { a.duration_secs.min(break_secs.max(1)) } else { break_secs };
    match &a.kind {
        ActionKind::None | ActionKind::OverlayOnly => {
            log_line(&format!("action {}: no system change (overlay handled by break screen)", a.kind.key()));
            vec![]
        }
        ActionKind::SleepMainMonitor => monitors_off(true, secs),
        ActionKind::SleepSecondaryMonitors => monitors_off(false, secs),
        ActionKind::SleepAllMonitors => {
            let ok = monitor_power(2);
            log_line(&format!("action sleep_all: SC_MONITORPOWER off sent -> {ok}"));
            if ok { vec![Undo::AllMonitorsOn] } else { vec![] }
        }
        ActionKind::Dim => {
            let ok = crate::blackout::break_overlay("all", secs, true, "", "");
            log_line(&format!("action dim: dim overlay all monitors {secs} s -> {ok}"));
            if ok { vec![Undo::Overlay("all".into())] } else { vec![] }
        }
        ActionKind::LockWorkstation => {
            // SAFETY: plain Win32 call without pointers.
            let ok = unsafe { LockWorkStation() } != 0;
            log_line(&format!("action lock: LockWorkStation -> {ok} (unlock is done by the user)"));
            vec![]
        }
        ActionKind::MuteAudio => {
            let ok = key_tap(&[VK_VOLUME_MUTE]);
            log_line(&format!("action mute: VK_VOLUME_MUTE sent -> {ok}"));
            if ok { vec![Undo::Unmute] } else { vec![] }
        }
        ActionKind::PauseMedia => {
            let ok = key_tap(&[VK_MEDIA_PLAY_PAUSE]);
            log_line(&format!("action pause_media: VK_MEDIA_PLAY_PAUSE sent -> {ok}"));
            if ok { vec![Undo::ResumeMedia] } else { vec![] }
        }
        ActionKind::MinimizeAllWindows => {
            let ok = key_tap(&[VK_LWIN, 0x4D]);
            log_line(&format!("action minimize_all: Win+M sent -> {ok}"));
            if ok { vec![Undo::RestoreWindows] } else { vec![] }
        }
        ActionKind::CustomCommand(c) => {
            let r = std::process::Command::new("cmd").args(["/C", c]).spawn();
            log_line(&format!("action custom: `{c}` -> {}", match &r { Ok(ch) => format!("started pid {}", ch.id()), Err(e) => format!("failed {e}") }));
            vec![]
        }
    }
}

/// DDC/CI off for the primary (`main`) or every non-primary monitor; overlay fallback.
fn monitors_off(main: bool, secs: u32) -> Vec<Undo> {
    let what = if main { "sleep_main" } else { "sleep_secondary" };
    let mons: Vec<_> = sys::monitors().into_iter().filter(|m| m.primary == main).collect();
    if mons.is_empty() {
        log_line(&format!("action {what}: no matching monitor"));
    }
    let mut undo = Vec::new();
    for m in mons {
        let before = sys::ddc_get_power(&m.name);
        if sys::ddc_set_power(&m.name, 4) {
            if !sys::is_mock() {
                std::thread::sleep(Duration::from_millis(400));
            }
            let mut now = sys::ddc_get_power(&m.name);
            if now == Some(1) && !sys::is_mock() {
                std::thread::sleep(Duration::from_millis(1200));
                now = sys::ddc_get_power(&m.name);
            }
            log_line(&format!("action {what}: {} DDC/CI off accepted, power before {before:?}, answers {now:?}", m.name));
            undo.push(Undo::DdcOn(m.name.clone()));
            if now == Some(1) {
                // panel still reports "on": make the screen dark anyway
                let ok = crate::blackout::break_overlay(&m.name, secs, false, "", "");
                log_line(&format!("action {what}: {} still reports on, black overlay {secs} s -> {ok}", m.name));
                if ok {
                    undo.push(Undo::Overlay(m.name.clone()));
                }
            }
        } else {
            let ok = crate::blackout::break_overlay(&m.name, secs, false, "", "");
            log_line(&format!("action {what}: {} DDC/CI rejected, black overlay fallback {secs} s -> {ok}", m.name));
            if ok {
                undo.push(Undo::Overlay(m.name.clone()));
            }
        }
    }
    undo
}

fn restore(u: &Undo) {
    match u {
        Undo::DdcOn(d) => {
            let ok = sys::ddc_set_power(d, 1);
            if !sys::is_mock() {
                std::thread::sleep(Duration::from_millis(400));
            }
            log_line(&format!("action restore: {d} DDC/CI on -> {ok}, answers {:?}", sys::ddc_get_power(d)));
        }
        Undo::Overlay(d) => {
            crate::blackout::cancel(d);
            log_line(&format!("action restore: overlay {d} cancelled"));
        }
        Undo::AllMonitorsOn => {
            let ok = monitor_power(-1);
            // a tiny mouse move wakes panels that ignore SC_MONITORPOWER on
            let m = mouse_nudge();
            log_line(&format!("action restore: SC_MONITORPOWER on -> {ok}, mouse nudge -> {m}"));
        }
        Undo::Unmute => log_line(&format!("action restore: VK_VOLUME_MUTE sent -> {}", key_tap(&[VK_VOLUME_MUTE]))),
        Undo::ResumeMedia => log_line(&format!("action restore: VK_MEDIA_PLAY_PAUSE sent -> {}", key_tap(&[VK_MEDIA_PLAY_PAUSE]))),
        Undo::RestoreWindows => log_line(&format!("action restore: Win+Shift+M sent -> {}", key_tap(&[VK_LWIN, VK_SHIFT, 0x4D]))),
    }
}

/// SC_MONITORPOWER to all top-level windows: 2 = off, -1 = on.
fn monitor_power(state: isize) -> bool {
    if sys::is_mock() {
        sys::mock_log(&format!("SC_MONITORPOWER {state}"));
        return true;
    }
    // PostMessage: a SendMessage broadcast blocks on every hung top-level window.
    // SAFETY: plain broadcast post, no pointers.
    unsafe { PostMessageW(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER as usize, state) != 0 }
}

fn ki(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    }
}

/// Press keys in order, release in reverse.
fn key_tap(keys: &[u16]) -> bool {
    if sys::is_mock() {
        sys::mock_log(&format!("keys {keys:?}"));
        return true;
    }
    let mut v: Vec<INPUT> = keys.iter().map(|k| ki(*k, false)).collect();
    v.extend(keys.iter().rev().map(|k| ki(*k, true)));
    // SAFETY: v is a valid INPUT array of the given length.
    let n = unsafe { SendInput(v.len() as u32, v.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    n as usize == v.len()
}

fn mouse_nudge() -> bool {
    if sys::is_mock() {
        return true;
    }
    let mk = |dx: i32| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx, dy: 0, mouseData: 0, dwFlags: MOUSEEVENTF_MOVE, time: 0, dwExtraInfo: 0 } },
    };
    let v = [mk(1), mk(-1)];
    // SAFETY: v is a valid INPUT array.
    unsafe { SendInput(2, v.as_ptr(), std::mem::size_of::<INPUT>() as i32) == 2 }
}

/// CLI `breakguard break-actions "<actions text>" <break secs>`: Break now with these
/// actions (no UI), wait the break, restore. Exit 0.
pub fn run_cli(args: &[String]) -> i32 {
    let list = match args.first() {
        Some(t) if t != "configured" => actions_from_text(t),
        _ => configured(),
    };
    let secs: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10).clamp(1, 3600);
    log_line(&format!("break now (cli): {} for {secs} s", crate::break_action::actions_to_text(&list)));
    start(list, secs);
    std::thread::sleep(Duration::from_secs(u64::from(secs)));
    end();
    std::thread::sleep(Duration::from_millis(300));
    log_line("break now (cli): done");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_runs_and_restores_every_action() {
        std::env::set_var("BREAKGUARD_DDC_MOCK", std::env::temp_dir().join("bg-exec-mock.txt").to_string_lossy().to_string());
        let undo = run_one(&BreakAction::new(ActionKind::MuteAudio, 0), 5);
        assert_eq!(undo, vec![Undo::Unmute]);
        let undo = run_one(&BreakAction::new(ActionKind::SleepAllMonitors, 0), 5);
        assert_eq!(undo, vec![Undo::AllMonitorsOn]);
        assert!(run_one(&BreakAction::new(ActionKind::None, 0), 5).is_empty());
    }
}
