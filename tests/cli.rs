//! Process-level QA tests. Every run uses BREAKGUARD_DDC_MOCK + BREAKGUARD_DATA_DIR (fresh temp dirs).
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

static N: AtomicU32 = AtomicU32::new(0);

struct Env {
    dir: PathBuf,
    mock: PathBuf,
}
impl Env {
    fn new() -> Env {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("bgqa-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        Env { mock: dir.join("mock.log"), dir }
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_breakguard"));
        c.args(args)
            .env("BREAKGUARD_DDC_MOCK", &self.mock)
            .env("BREAKGUARD_DATA_DIR", &self.dir)
            // Second monitor only, never the primary one: override with BREAKGUARD_TEST_MONITOR="x,y,w,h" and BREAKGUARD_TEST_POS="x,y" (logical).
            .env("BREAKGUARD_ONLY_MONITOR", std::env::var("BREAKGUARD_TEST_MONITOR").unwrap_or_else(|_| "3840,0,3840,2160".into()))
            .env("BREAKGUARD_WINDOW_POS", std::env::var("BREAKGUARD_TEST_POS").unwrap_or_else(|_| "2627,66".into()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    }
    fn mock_text(&self) -> String {
        std::fs::read_to_string(&self.mock).unwrap_or_default()
    }
    fn settings(&self) -> PathBuf {
        self.dir.join("settings.ini")
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Run with a hard timeout; kills only the spawned child. Returns (code, output, elapsed).
fn run(mut c: Command, limit: Duration) -> (Option<i32>, String, Duration) {
    let t = Instant::now();
    let mut ch: Child = c.spawn().expect("spawn");
    loop {
        if let Some(st) = ch.try_wait().unwrap() {
            let o = ch.wait_with_output().unwrap();
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s += &String::from_utf8_lossy(&o.stderr);
            return (st.code(), s, t.elapsed());
        }
        if t.elapsed() > limit {
            let _ = ch.kill();
            let _ = ch.wait();
            return (None, "TIMEOUT".into(), t.elapsed());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn no_panic(out: &str) {
    assert!(!out.contains("panicked"), "panic: {out}");
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

fn no_real_display(m: &str) {
    for l in m.lines() {
        assert!(!(l.contains("DISPLAY") && !l.contains("DISPLAYMOCK")), "real display name in mock log: {l}");
    }
}

#[test]
fn selftest_ok() {
    let e = Env::new();
    let (code, out, _) = run(e.cmd(&["selftest"]), Duration::from_secs(30));
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("SELFTEST OK"), "{out}");
}

#[test]
fn version_prints() {
    let e = Env::new();
    let (code, out, _) = run(e.cmd(&["--version"]), Duration::from_secs(10));
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("0."), "{out}");
}

#[test]
fn version_and_help_survive_closed_stdout_and_stderr() {
    let e = Env::new();
    for flag in ["--version", "--help"] {
        let st = e.cmd(&[flag]).stdout(Stdio::null()).stderr(Stdio::null()).status().expect("spawn");
        assert_eq!(st.code(), Some(0), "{flag} must not panic when its output goes nowhere");
    }
}

#[test]
fn help_prints_usage_and_does_not_start_the_gui() {
    let e = Env::new();
    for flag in ["--help", "-h"] {
        let (code, out, _) = run(e.cmd(&[flag]), Duration::from_secs(10));
        assert_eq!(code, Some(0), "{flag}: {out}");
        assert!(out.contains("Usage: breakguard"), "{flag}: {out}");
    }
}

#[test]
fn mock_proves_no_real_hardware_path() {
    let e = Env::new();
    let (code, out, _) = run(e.cmd(&["selftest"]), Duration::from_secs(30));
    assert_eq!(code, Some(0), "{out}");
    no_real_display(&e.mock_text());
}

#[test]
fn blackout_honors_deadline() {
    let e = Env::new();
    let (code, out, el) = run(
        e.cmd(&["blackout", "--monitor", r"\\.\DISPLAYMOCK2", "--secs", "2"]),
        Duration::from_secs(15),
    );
    no_panic(&out);
    assert_eq!(code, Some(0), "{out}");
    assert!(el >= Duration::from_millis(1000) && el <= Duration::from_secs(4), "elapsed {el:?}");
}

#[test]
fn blackout_all_and_options() {
    let e = Env::new();
    let (code, out, el) = run(
        e.cmd(&["blackout", "--monitor", "all", "--secs", "1", "--alpha", "200", "--tint", "10,20,30"]),
        Duration::from_secs(15),
    );
    no_panic(&out);
    assert_eq!(code, Some(0), "{out}");
    assert!(el <= Duration::from_secs(4), "{el:?}");
}

#[test]
fn blackout_bad_args_never_panic() {
    let cases: Vec<Vec<&str>> = vec![
        vec!["blackout"],
        vec!["blackout", "--monitor"],
        vec!["blackout", "--secs"],
        vec!["blackout", "--secs", "abc"],
        vec!["blackout", "--secs", "-5", "--monitor", "all"],
        vec!["blackout", "--secs", "0", "--monitor", "all"],
        vec!["blackout", "--secs", "99999999999999999999", "--monitor", "all"],
        vec!["blackout", "--monitor", "all", "--secs", "1", "--alpha", "999"],
        vec!["blackout", "--monitor", "all", "--secs", "1", "--alpha", "-1"],
        vec!["blackout", "--monitor", "all", "--secs", "1", "--tint", "x,y"],
        vec!["blackout", "--monitor", "all", "--secs", "1", "--tint", "1,2"],
        vec!["blackout", "--monitor", "all", "--secs", "1", "--tint", "999,999,999"],
        vec!["blackout", "--bogus", "--monitor", "all", "--secs", "1"],
        vec!["blackout", "--monitor", "", "--secs", "1"],
        vec!["blackout", "--monitor", "nonexistent", "--secs", "1"],
    ];
    for a in cases {
        let e = Env::new();
        // A legit long overlay may outlive the cap; it is killed (child only). Crash codes are not allowed.
        let (code, out, _) = run(e.cmd(&a), Duration::from_secs(6));
        no_panic(&out);
        if let Some(c) = code {
            assert!((0..=2).contains(&c), "args {a:?} exit {c}: {out}");
        }
    }
}

#[test]
fn blackout_garbage_secs_does_not_run_forever() {
    // negative / zero / non-numeric secs must end fast (usage error or clamp), not hang.
    for s in ["abc", "-5", "0"] {
        let e = Env::new();
        let (code, out, el) = run(e.cmd(&["blackout", "--monitor", "all", "--secs", s]), Duration::from_secs(6));
        no_panic(&out);
        assert!(code.is_some(), "--secs {s} hung");
        assert!(el < Duration::from_secs(1), "--secs {s} took {el:?}");
    }
}

#[test]
fn selftest_refuses_without_mock() {
    // Safety guard: without BREAKGUARD_DDC_MOCK selftest must refuse (rc 2) and touch no hardware.
    let e = Env::new();
    let mut c = e.cmd(&["selftest"]);
    c.env_remove("BREAKGUARD_DDC_MOCK");
    let (code, out, _) = run(c, Duration::from_secs(10));
    no_panic(&out);
    assert_eq!(code, Some(2), "{out}");
    assert!(!e.mock.exists());
}

#[test]
fn monitor_name_injection_rejected_safely() {
    let long = "A".repeat(5000);
    let names: Vec<String> = vec![
        "; calc".into(),
        "& calc.exe".into(),
        "| whoami".into(),
        "$(calc)".into(),
        "`calc`".into(),
        "../../..".into(),
        "\\\\.\\DISPLAY1; calc".into(),
        "мониторǼ😀".into(),
        long,
    ];
    for n in names {
        let e = Env::new();
        let (code, out, _) = run(e.cmd(&["blackout", "--monitor", &n, "--secs", "1"]), Duration::from_secs(8));
        no_panic(&out);
        assert!(code.is_some(), "blackout hang for {:?}", &n[..n.len().min(20)]);
        let (code2, out2, _) = run(e.cmd(&["helper-on", "--monitor", &n, "--at", "0"]), Duration::from_secs(12));
        no_panic(&out2);
        assert!(code2.is_some(), "helper hang for {:?}", &n[..n.len().min(20)]);
        for l in e.mock_text().lines() {
            assert!(!l.contains("DISPLAY1;"), "injected name used raw: {l}");
        }
    }
}

#[test]
fn helper_reenables_after_deadline() {
    let e = Env::new();
    let at = (now() + 2).to_string();
    let (code, out, el) = run(
        e.cmd(&["helper-on", "--monitor", r"\\.\DISPLAYMOCK2", "--at", &at]),
        Duration::from_secs(20),
    );
    no_panic(&out);
    assert_eq!(code, Some(0), "{out}");
    assert!(el <= Duration::from_secs(8), "{el:?}");
    let m = e.mock_text();
    assert!(m.contains("DISPLAYMOCK2"), "mock log: {m}");
}

#[test]
fn helper_past_deadline_acts_immediately() {
    let e = Env::new();
    let (code, out, el) = run(
        e.cmd(&["helper-on", "--monitor", r"\\.\DISPLAYMOCK1", "--at", "1"]),
        Duration::from_secs(15),
    );
    no_panic(&out);
    assert_eq!(code, Some(0), "{out}");
    assert!(el <= Duration::from_secs(6), "{el:?}");
    assert!(e.mock_text().contains("DISPLAYMOCK1"));
}

#[test]
fn helper_bad_args_never_panic() {
    let cases: Vec<Vec<&str>> = vec![
        vec!["helper-on"],
        vec!["helper-on", "--monitor"],
        vec!["helper-on", "--at"],
        vec!["helper-on", "--monitor", "x", "--at", "abc"],
        vec!["helper-on", "--monitor", "x", "--at", "-1"],
        vec!["helper-on", "--monitor", "x", "--at", "99999999999999999999"],
        vec!["helper-on", "--monitor", "", "--at", "0"],
    ];
    for a in cases {
        let e = Env::new();
        let (code, out, _) = run(e.cmd(&a), Duration::from_secs(8));
        no_panic(&out);
        if let Some(c) = code {
            assert!((0..=2).contains(&c), "{a:?} exit {c}: {out}");
        }
    }
}

#[test]
fn unknown_subcommand_no_panic() {
    for a in [vec!["bogus"], vec!["--help"], vec!["-x"]] {
        let e = Env::new();
        let (code, out, _) = run(e.cmd(&a), Duration::from_secs(5));
        no_panic(&out);
        if let Some(c) = code {
            assert!((0..=2).contains(&c), "{a:?} exit {c}");
        }
    }
}

fn selftest_with_settings(bytes: &[u8]) {
    let e = Env::new();
    std::fs::write(e.settings(), bytes).unwrap();
    let (code, out, _) = run(e.cmd(&["selftest"]), Duration::from_secs(30));
    no_panic(&out);
    assert_eq!(code, Some(0), "{out}");
    assert!(out.contains("SELFTEST OK"), "{out}");
}

#[test]
fn settings_corruption() {
    selftest_with_settings(b"");
    selftest_with_settings(&[0u8, 159, 146, 150, 0xff, 0xfe, 0, 1, 2, 3]);
    selftest_with_settings(&vec![b'x'; 5_000_000]);
    selftest_with_settings(b"work_min=2");
    selftest_with_settings(b"work_min");
    selftest_with_settings(b"=\n=\n==\n");
    selftest_with_settings(b"\xEF\xBB\xBFwork_min=30\r\nbreak_secs=20\r\n");
    selftest_with_settings(b"work_min=30\r\nwork_min=40\nwork_min=-1\nwork_min=99999999999999999999\n");
    selftest_with_settings(b"work_min=abc\nbreak_secs=-5\nwarn_secs=4294967296\nnight_level=999\nnight_start=99:99\nnight_end=x\nsleep_min=0\noff_mode=zzz\n");
    selftest_with_settings(b"scheds=garbage;;;|||\nsched=1,2,3\nselected=\n");
    let mut many = String::new();
    for i in 0..5000 {
        many += &format!("sched{i}=1,2,3\nselected=\\\\.\\DISPLAY{i}\n");
    }
    selftest_with_settings(many.as_bytes());
}

#[test]
fn settings_path_is_directory() {
    let e = Env::new();
    std::fs::create_dir_all(e.settings()).unwrap();
    // Unwritable settings path: selftest may legitimately report FAIL (exit 1), but must not panic/hang.
    let (code, out, _) = run(e.cmd(&["selftest"]), Duration::from_secs(30));
    no_panic(&out);
    assert!(matches!(code, Some(0) | Some(1)), "{code:?} {out}");
}

#[test]
fn log_rotation_bounded() {
    let e = Env::new();
    std::fs::write(e.dir.join("breakguard.log"), vec![b'a'; 600_000]).unwrap();
    // selftest does not log; helper-on logs 'turn on (timer)' lines.
    let (code, out, _) = run(
        e.cmd(&["helper-on", "--monitor", r"\\.\DISPLAYMOCK1", "--at", "1"]),
        Duration::from_secs(15),
    );
    no_panic(&out);
    assert_eq!(code, Some(0), "{out}");
    let mut total = 0u64;
    for f in std::fs::read_dir(&e.dir).unwrap().flatten() {
        let n = f.file_name().to_string_lossy().to_string();
        // rotated copy (.1) may hold the old oversized content; only the live log is bounded.
        if n == "breakguard.log" {
            let l = f.metadata().unwrap().len();
            assert!(l <= 300 * 1024, "{n} is {l} bytes");
            total += l;
        }
    }
    assert!(total <= 300 * 1024, "total {total}");
}

#[test]
fn gui_smoke_and_second_instance() {
    let e = Env::new();
    let mut ch = e.cmd(&["--demo-tour"]).spawn().expect("spawn gui");
    std::thread::sleep(Duration::from_secs(3));
    if let Some(st) = ch.try_wait().unwrap() {
        let o = ch.wait_with_output().unwrap();
        panic!("GUI died early: {st:?} {}", String::from_utf8_lossy(&o.stderr));
    }
    // second instance: must not crash, must not kill the first.
    let (code, out, _) = run(e.cmd(&[]), Duration::from_secs(2));
    no_panic(&out);
    if let Some(c) = code {
        assert!(c <= 2, "second instance code {c}");
    }
    // demo tour exits 0 on its own (~6s); else killed only as our child at 20s.
    let t = Instant::now();
    let st = loop {
        if let Some(s) = ch.try_wait().unwrap() {
            break Some(s);
        }
        if t.elapsed() > Duration::from_secs(20) {
            let _ = ch.kill();
            let _ = ch.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let st = st.expect("--demo-tour did not exit within 20 s");
    assert_eq!(st.code(), Some(0), "demo-tour exit code");
    no_real_display(&e.mock_text());
}

fn helper_deadlines(dir: &std::path::Path) -> Vec<i64> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|d| d.ok())
        .filter(|d| d.file_name().to_string_lossy().contains("helper"))
        .filter_map(|d| std::fs::read_to_string(d.path()).ok())
        .filter_map(|t| t.lines().nth(1).and_then(|l| l.trim().parse::<i64>().ok()))
        .collect()
}

#[test]
fn off_cli_chosen_monitors_and_bad_args() {
    for (mon, want, not) in [("1", "DISPLAYMOCK1", "DISPLAYMOCK2"), ("2", "DISPLAYMOCK2", "DISPLAYMOCK1")] {
        let e = Env::new();
        let (code, out, el) = run(e.cmd(&["off", "--monitor", mon, "--minutes", "1"]), Duration::from_secs(20));
        no_panic(&out);
        assert_eq!(code, Some(0), "{out}");
        assert!(el < Duration::from_secs(8), "{el:?}");
        let m = e.mock_text();
        assert!(m.contains(&format!("{want} 4 -> ok")), "mock log: {m}");
        assert!(!m.contains(&format!("{not} 4")), "other monitor touched: {m}");
        // the failsafe helper restores the monitor at now + minutes*60
        let mut ok = false;
        for _ in 0..40 {
            ok = helper_deadlines(&e.dir).iter().any(|at| (at - now() - 60).abs() <= 5);
            if ok {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(ok, "no helper with a 60 s deadline in {:?}", e.dir);
    }
    let e = Env::new();
    let (code, _, _) = run(e.cmd(&["off", "--monitor", "both", "--minutes", "1"]), Duration::from_secs(20));
    assert_eq!(code, Some(0));
    let m = e.mock_text();
    assert!(m.contains("DISPLAYMOCK1 4 -> ok") && m.contains("DISPLAYMOCK2 4 -> ok"), "{m}");
    let bads: [&[&str]; 6] = [
        &["off"],
        &["off", "--monitor", "3", "--minutes", "5"],
        &["off", "--monitor", "1", "--minutes", "0"],
        &["off", "--monitor", "1", "--minutes", "9999"],
        &["off", "--monitor", "1", "--minutes", "x"],
        &["off", "--monitor", "1", "--minutes", "5", "--mode", "zzz"],
    ];
    for bad in bads {
        let e = Env::new();
        let (code, out, el) = run(e.cmd(bad), Duration::from_secs(10));
        assert_eq!(code, Some(2), "{bad:?}: {out}");
        assert!(el < Duration::from_millis(500), "{el:?}");
        assert!(!e.mock_text().contains(" 4 "), "bad args touched a monitor");
    }
}

// ---- agent pause failsafe: killing the (mock) BreakGuard instance must resume the paused dummy

fn cpu_ticks(pid: u32) -> u64 {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        assert!(!h.is_null(), "dummy gone");
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        CloseHandle(h);
        let f = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        f(k) + f(u)
    }
}

fn cpu_rate(pid: u32, ms: u64) -> u64 {
    let a = cpu_ticks(pid);
    std::thread::sleep(Duration::from_millis(ms));
    cpu_ticks(pid) - a
}

#[test]
fn killing_mock_breakguard_resumes_paused_agent() {
    use std::io::{BufRead, BufReader};
    let e = Env::new();
    let mut dummy = Command::new("powershell")
        .args(["-NoProfile", "-Command", "$e=[Environment]::TickCount+60000; while([Environment]::TickCount -lt $e){ $x=1+1 }"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = dummy.id();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(cpu_rate(pid, 800) > 2_000_000, "dummy must be busy first");
    let mut bg = e.cmd(&["pause-agents", "--pids", &pid.to_string(), "--hold-secs", "120"]).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(bg.stdout.take().unwrap()).read_line(&mut line).unwrap();
    assert!(line.contains("PAUSED 1"), "{line}");
    std::thread::sleep(Duration::from_millis(400));
    assert!(cpu_rate(pid, 1000) < 150_000, "dummy must be paused while the mock instance lives");
    assert!(e.dir.join("paused-agents.txt").exists(), "state file written");
    bg.kill().unwrap(); // hard kill of only our own mock instance: no cleanup code runs
    let _ = bg.wait();
    let mut ok = false;
    for _ in 0..40 {
        if cpu_rate(pid, 500) > 2_000_000 {
            ok = true;
            break;
        }
    }
    let _ = dummy.kill();
    let _ = dummy.wait();
    assert!(ok, "failsafe helper must resume the dummy after the instance was killed");
}
