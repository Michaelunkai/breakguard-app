//! Read-only discovery of running Claude Code agents: `~/.claude/sessions/<pid>.json` (one per CLI
//! process) cross-checked against the live process table, named like the Claude app names them.
use crate::sessions::{Json, Roots};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub pid: u32,
    pub session_id: String,
    pub name: String,
    pub cwd: String,
    /// Process start (unix ms) as recorded by Claude, 0 when unknown.
    pub started_ms: i64,
}

/// Is this pid a live process that started at `proc_start` (FILETIME ticks, 0 = unknown)?
pub type AliveFn<'a> = &'a dyn Fn(u32, u64) -> bool;

/// Parse a JSON file, tolerating a UTF-8 BOM (PowerShell writes one).
fn read_json(p: &Path) -> Option<Json> {
    let t = std::fs::read_to_string(p).ok()?;
    Json::parse(t.trim_start_matches('\u{feff}'))
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
        .unwrap_or_default()
}

/// What one desktop session file says: (its own `local_…` id, CLI session ids it wraps, title).
type DeskInfo = Option<(String, Vec<String>, String)>;

/// Desktop session files already parsed, keyed by path with their modified time: a scan re-reads
/// only files that changed, so a rename in the Claude sidebar shows up on the next scan.
fn desk_cache() -> &'static std::sync::Mutex<HashMap<PathBuf, (std::time::SystemTime, DeskInfo)>> {
    static C: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf, (std::time::SystemTime, DeskInfo)>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

fn desk_info(p: &Path) -> DeskInfo {
    let j = read_json(p)?;
    let t = j.gs("title").filter(|t| !t.is_empty())?.to_string();
    let host = j.gs("sessionId").unwrap_or("").to_string();
    let clis = ["cliSessionId", "lastAssistantCliSessionId"].iter().filter_map(|k| j.gs(k)).filter(|s| !s.is_empty()).map(str::to_string).collect();
    Some((host, clis, t))
}

/// Titles the Claude desktop app shows, keyed by the CLI session ids they wrap and by the desktop
/// session's own `local_…` id (the `hostSessionId` of a `<pid>.json`).
pub fn desktop_titles(roots: &Roots) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut cache = desk_cache().lock().unwrap_or_else(|e| e.into_inner());
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![(roots.desktop.clone(), 0u32)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 3 {
                    stack.push((p, depth + 1));
                }
            } else if p.extension().is_some_and(|x| x == "json") {
                let mt = e.metadata().and_then(|md| md.modified()).unwrap_or(std::time::UNIX_EPOCH);
                let info = match cache.get(&p) {
                    Some((t, i)) if *t == mt => i.clone(),
                    _ => {
                        let i = desk_info(&p);
                        cache.insert(p.clone(), (mt, i.clone()));
                        i
                    }
                };
                seen.insert(p);
                if let Some((host, clis, t)) = info {
                    for c in clis {
                        m.entry(c).or_insert_with(|| t.clone());
                    }
                    if !host.is_empty() {
                        m.insert(host, t);
                    }
                }
            }
        }
    }
    cache.retain(|p, _| seen.contains(p));
    m
}

/// Read a `<pid>.json`; one quick retry covers a file caught half-written by Claude.
fn read_pid_file(p: &Path) -> Option<Json> {
    read_json(p).or_else(|| {
        std::thread::sleep(std::time::Duration::from_millis(15));
        read_json(p)
    })
}

/// Name order: desktop title, the CLI's own name, project folder, short id.
pub fn pick_name(title: Option<&str>, cli_name: &str, cwd: &str, sid: &str) -> String {
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        return t.to_string();
    }
    if !cli_name.is_empty() {
        return cli_name.to_string();
    }
    if let Some(f) = Path::new(cwd).file_name().and_then(|n| n.to_str()).filter(|f| !f.is_empty()) {
        return f.to_string();
    }
    sid.chars().take(8).collect()
}

/// The pid files of running sessions (same filter for the Live tab and the status bar count).
fn live_pid_files(roots: &Roots, alive: AliveFn) -> Vec<(u32, Json)> {
    let mut out = Vec::new();
    for p in files_in(&roots.cli) {
        let Some(j) = read_pid_file(&p) else { continue };
        let (Some(_), Some(pid)) = (j.gs("sessionId"), j.get("pid").and_then(Json::num)) else { continue };
        // SDK children (tools and helpers that drive Claude programmatically) are not sessions the
        // user has open in Claude; only the desktop app's and terminal sessions are listed.
        if j.gs("entrypoint") == Some("sdk-cli") {
            continue;
        }
        let pid = pid as u32;
        let start: u64 = j.gs("procStart").and_then(|s| s.parse().ok()).unwrap_or(0);
        if pid == 0 || !alive(pid, start) {
            continue;
        }
        out.push((pid, j));
    }
    out
}

/// How many sessions [`discover`] would list, without reading the desktop titles (cheap, every second).
pub fn count_live(roots: &Roots, alive: AliveFn) -> usize {
    live_pid_files(roots, alive).len()
}

/// Running agents, newest first. Never writes or touches a process.
pub fn discover(roots: &Roots, alive: AliveFn) -> Vec<Agent> {
    let titles = desktop_titles(roots);
    let mut out: Vec<(i64, Agent)> = Vec::new();
    for (pid, j) in live_pid_files(roots, alive) {
        let Some(sid) = j.gs("sessionId") else { continue };
        let cwd = j.gs("cwd").unwrap_or("");
        let title = j.gs("hostSessionId").and_then(|h| titles.get(h)).or_else(|| titles.get(sid));
        let name = pick_name(title.map(String::as_str), j.gs("name").unwrap_or(""), cwd, sid);
        let started = j.get("startedAt").and_then(Json::num).unwrap_or(0.0) as i64;
        out.push((started, Agent { pid, session_id: sid.to_string(), name, cwd: cwd.to_string(), started_ms: started }));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.pid.cmp(&b.1.pid)));
    out.into_iter().map(|(_, a)| a).collect()
}

/// Real liveness: the pid exists and its creation time matches the recorded start (guards pid reuse).
pub fn process_alive(pid: u32, proc_start: u64) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let running = GetExitCodeProcess(h, &mut code) != 0 && code == 259; // STILL_ACTIVE
        let mut ok = running;
        if running && proc_start != 0 {
            let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            let (mut c, mut e, mut k, mut u) = (z, z, z, z);
            if GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0 {
                let created = ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64;
                // recorded value is the same clock to within a few seconds
                ok = created.abs_diff(proc_start) < 50_000_000;
            }
        }
        CloseHandle(h);
        ok
    }
}

// ------------------------------------------------------------ suspend / resume

#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(h: windows_sys::Win32::Foundation::HANDLE) -> i32;
    fn NtResumeProcess(h: windows_sys::Win32::Foundation::HANDLE) -> i32;
}

/// (pid, parent pid) of every process, from one Toolhelp snapshot. Read-only.
pub fn process_table() -> Vec<(u32, u32)> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    let mut v = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap as isize == -1 {
            return v;
        }
        let mut e: PROCESSENTRY32W = std::mem::zeroed();
        e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            v.push((e.th32ProcessID, e.th32ParentProcessID));
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    v
}

/// `root` plus all its descendants, parents before children. Pid 0/4 and this process are never included.
pub fn tree_of(root: u32, table: &[(u32, u32)]) -> Vec<u32> {
    let me = std::process::id();
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() {
        let cur = out[i];
        for &(pid, ppid) in table {
            if ppid == cur && pid != cur && !out.contains(&pid) {
                out.push(pid);
            }
        }
        i += 1;
    }
    out.retain(|&p| p > 4 && p != me);
    out
}

fn nt_apply(pid: u32, suspend: bool) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};
    unsafe {
        let h = OpenProcess(PROCESS_SUSPEND_RESUME, 0, pid);
        if h.is_null() {
            return false;
        }
        let st = if suspend { NtSuspendProcess(h) } else { NtResumeProcess(h) };
        CloseHandle(h);
        st >= 0
    }
}

/// Suspend the whole tree under `root` (never kills). Returns the pids actually suspended.
#[cfg(test)]
pub fn suspend_tree(root: u32) -> Vec<u32> {
    tree_of(root, &process_table()).into_iter().filter(|&p| nt_apply(p, true)).collect()
}

/// Resume these pids (children first is not needed: resume is per process).
pub fn resume_pids(pids: &[u32]) -> usize {
    pids.iter().filter(|&&p| nt_apply(p, false)).count()
}

/// Total CPU time (kernel + user, 100 ns ticks) a process has used; None if it is gone.
#[cfg(test)]
pub fn cpu_ticks(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0;
        CloseHandle(h);
        let f = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        ok.then(|| f(k) + f(u))
    }
}

// ------------------------------------------------------------ pause manager + failsafe

/// Creation time (FILETIME ticks) of a process; None if it is gone.
pub fn creation_ticks(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0;
        CloseHandle(h);
        ok.then(|| ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64)
    }
}

pub fn state_path() -> PathBuf {
    crate::engine::data_dir().join("paused-agents.txt")
}

/// State file: line 1 = owner pid, then one `pid creation_ticks` line per suspended process.
fn write_state(path: &Path, owner: u32, pids: &[u32]) {
    if pids.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let mut t = format!("{owner}\n");
    for &p in pids {
        t.push_str(&format!("{p} {}\n", creation_ticks(p).unwrap_or(0)));
    }
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
    let _ = std::fs::write(path, t);
}

/// Resume every process listed in a state file (pid reuse guarded by creation time), then delete it.
pub fn resume_from_state(path: &Path) -> usize {
    let Ok(t) = std::fs::read_to_string(path) else { return 0 };
    let mut n = 0;
    for l in t.lines().skip(1) {
        let mut it = l.split_whitespace();
        let (Some(pid), Some(cr)) = (it.next().and_then(|x| x.parse::<u32>().ok()), it.next().and_then(|x| x.parse::<u64>().ok())) else { continue };
        if cr != 0 && creation_ticks(pid) != Some(cr) {
            continue;
        }
        if nt_apply(pid, false) {
            n += 1;
        }
    }
    let _ = std::fs::remove_file(path);
    n
}

/// `breakguard helper-resume <owner_pid> <state file>`: detached failsafe. Waits for the owner to exit
/// (crash, kill, quit) then resumes everything still listed. Gives up waiting after 6 hours.
pub fn run_resume_helper(args: &[String]) -> i32 {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
    let (Some(owner), Some(path)) = (args.first().and_then(|a| a.parse::<u32>().ok()), args.get(1)) else {
        eprintln!("helper-resume <owner_pid> <state file>");
        return 2;
    };
    let path = PathBuf::from(path);
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, owner);
        if !h.is_null() {
            WaitForSingleObject(h, 6 * 3600 * 1000);
            CloseHandle(h);
        }
    }
    // the owner may have resumed and removed the list itself; nothing left then
    resume_from_state(&path);
    0
}

fn spawn_helper(owner: u32, path: &Path) -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    let Ok(exe) = std::env::current_exe() else { return false };
    for flags in [0x0100_0208u32, 0x0000_0208u32] {
        if Command::new(&exe)
            .args(["helper-resume".to_string(), owner.to_string(), path.to_string_lossy().into_owned()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(flags)
            .spawn()
            .is_ok()
        {
            return true;
        }
    }
    false
}

/// What BreakGuard currently holds suspended, per agent session. Suspends, never kills.
#[derive(Default)]
pub struct Pauser {
    held: Vec<(String, Vec<u32>)>,
    state: Option<PathBuf>,
    helper_armed: bool,
}

impl Pauser {
#[cfg(test)]
    pub fn with_state(path: PathBuf) -> Pauser {
        Pauser { held: Vec::new(), state: Some(path), helper_armed: false }
    }
    fn path(&self) -> PathBuf {
        self.state.clone().unwrap_or_else(state_path)
    }
    pub fn is_paused(&self, session_id: &str) -> bool {
        self.held.iter().any(|(s, _)| s == session_id)
    }
    pub fn any_paused(&self) -> bool {
        !self.held.is_empty()
    }
    fn flush(&mut self) {
        let all: Vec<u32> = self.held.iter().flat_map(|(_, p)| p.iter().copied()).collect();
        let path = self.path();
        write_state(&path, std::process::id(), &all);
        if !all.is_empty() && !self.helper_armed {
            self.helper_armed = spawn_helper(std::process::id(), &path);
        }
    }
    /// Suspend these agents' process trees; returns how many agents became paused.
    pub fn pause(&mut self, agents: &[Agent]) -> usize {
        let table = process_table();
        let mut n = 0;
        for a in agents {
            if self.is_paused(&a.session_id) {
                continue;
            }
            let pids: Vec<u32> = tree_of(a.pid, &table).into_iter().filter(|&p| nt_apply(p, true)).collect();
            if !pids.is_empty() {
                self.held.push((a.session_id.clone(), pids));
                n += 1;
            }
        }
        self.flush();
        n
    }
    pub fn resume_session(&mut self, session_id: &str) -> bool {
        let Some(i) = self.held.iter().position(|(s, _)| s == session_id) else { return false };
        let (_, pids) = self.held.remove(i);
        resume_pids(&pids);
        self.flush();
        true
    }
    pub fn resume_all(&mut self) -> usize {
        let n = self.held.len();
        for (_, pids) in self.held.drain(..) {
            resume_pids(&pids);
        }
        self.flush();
        n
    }
}

/// Startup recovery: BreakGuard died earlier while agents were suspended; wake them.
pub fn recover_on_start() -> usize {
    resume_from_state(&state_path())
}

/// `breakguard pause-agents --pids 1,2 [--hold-secs N]`: pauses the process trees of those pids exactly
/// like a break would, then holds. Stand-in "BreakGuard instance" for failsafe tests and manual checks.
pub fn run_pause_cli(args: &[String]) -> i32 {
    let mut pids: Vec<u32> = Vec::new();
    let mut hold = 60u64;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--pids" => {
                pids = args.get(i + 1).map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
                i += 1;
            }
            "--hold-secs" => {
                hold = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(60);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    if pids.is_empty() {
        eprintln!("pause-agents --pids 1,2 [--hold-secs N]");
        return 2;
    }
    let agents: Vec<Agent> = pids.iter().map(|&p| Agent { pid: p, session_id: format!("cli-{p}"), name: format!("pid {p}"), cwd: String::new(), started_ms: 0 }).collect();
    let mut pz = Pauser::default();
    let n = pz.pause(&agents);
    println!("PAUSED {n}");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    std::thread::sleep(std::time::Duration::from_secs(hold));
    pz.resume_all();
    println!("RESUMED");
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::roots_with;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bg-agents-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn discovers_live_agents_with_correct_names() {
        let d = tmp("disc");
        let cli = d.join("sessions");
        let desk = d.join("claude-code-sessions").join("a").join("o");
        std::fs::create_dir_all(&cli).unwrap();
        std::fs::create_dir_all(&desk).unwrap();
        std::fs::write(cli.join("100.json"), r#"{"pid":100,"sessionId":"s-a","cwd":"C:\\w\\alpha","startedAt":5,"name":"derived-a"}"#).unwrap();
        std::fs::write(cli.join("200.json"), r#"{"pid":200,"sessionId":"s-b","cwd":"C:\\w\\beta","startedAt":9}"#).unwrap();
        std::fs::write(cli.join("300.json"), r#"{"pid":300,"sessionId":"s-dead","cwd":"C:\\w\\dead","startedAt":1}"#).unwrap();
        std::fs::write(cli.join("400.json"), r#"{"pid":400,"sessionId":"abcdefghijkl","cwd":"","startedAt":2}"#).unwrap();
        std::fs::write(cli.join("junk.json"), "not json").unwrap();
        std::fs::write(desk.join("local_1.json"), r#"{"sessionId":"local_1","cliSessionId":"s-a","title":"My Title"}"#).unwrap();
        let alive = |pid: u32, _s: u64| pid != 300;
        let v = discover(&roots_with(Some(&d)), &alive);
        let got: Vec<(u32, &str)> = v.iter().map(|a| (a.pid, a.name.as_str())).collect();
        assert_eq!(got, vec![(200, "beta"), (100, "My Title"), (400, "abcdefgh")]);
        assert_eq!(pick_name(None, "derived", "C:\\x", "id"), "derived");
    }

    #[test]
    fn desktop_sessions_named_by_host_id_sdk_skipped_and_renames_seen() {
        let d = tmp("disc2");
        let cli = d.join("sessions");
        let desk = d.join("claude-code-sessions").join("u1").join("u2");
        std::fs::create_dir_all(&cli).unwrap();
        std::fs::create_dir_all(&desk).unwrap();
        // desktop session whose CLI id changed since the title file was written: matched by hostSessionId
        std::fs::write(cli.join("10.json"), r#"{"pid":10,"sessionId":"new-cli","hostSessionId":"local_9","entrypoint":"claude-desktop","cwd":"C:\\w\\p","startedAt":3,"name":"old"}"#).unwrap();
        std::fs::write(cli.join("20.json"), r#"{"pid":20,"sessionId":"sdk","entrypoint":"sdk-cli","cwd":"C:\\w\\q","startedAt":4}"#).unwrap();
        std::fs::write(cli.join("30.json"), r#"{"pid":30,"sessionId":"term","entrypoint":"cli","cwd":"C:\\w\\term","startedAt":1}"#).unwrap();
        let f = desk.join("local_9.json");
        std::fs::write(&f, r#"{"sessionId":"local_9","cliSessionId":"old-cli","title":"break"}"#).unwrap();
        let alive = |_: u32, _: u64| true;
        let names = || discover(&roots_with(Some(&d)), &alive).into_iter().map(|a| (a.pid, a.name)).collect::<Vec<_>>();
        assert_eq!(names(), vec![(10, "break".to_string()), (30, "term".to_string())]);
        // rename in the sidebar: the next scan shows the new title
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&f, r#"{"sessionId":"local_9","cliSessionId":"old-cli","title":"BreakGuard live logs"}"#).unwrap();
        let t = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(t).unwrap();
        assert_eq!(names()[0].1, "BreakGuard live logs");
    }

    #[test]
    fn own_process_is_alive_and_wrong_start_is_not() {
        let me = std::process::id();
        assert!(process_alive(me, 0));
        assert!(!process_alive(me, 1), "recorded start far from real start = reused pid");
        assert!(!process_alive(0x7fff_fff0, 0));
    }

    /// Dummy busy-loop child (own process tree: powershell -> its own busy loop) stops accruing CPU
    /// while suspended and continues after resume. Only the child this test started is touched.
    #[test]
    fn suspend_stops_cpu_and_resume_continues() {
        use std::time::Duration;
        let mut ch = busy_child();
        let pid = ch.id();
        std::thread::sleep(Duration::from_millis(1500));
        let a0 = cpu_ticks(pid).unwrap();
        std::thread::sleep(Duration::from_millis(800));
        let a1 = cpu_ticks(pid).unwrap();
        assert!(a1 - a0 > 2_000_000, "dummy must be busy before pause: {}", a1 - a0);
        let paused = suspend_tree(pid);
        assert!(paused.contains(&pid), "{paused:?}");
        std::thread::sleep(Duration::from_millis(300));
        let b0 = cpu_ticks(pid).unwrap();
        std::thread::sleep(Duration::from_millis(1200));
        let b1 = cpu_ticks(pid).unwrap();
        assert!(b1 - b0 < 150_000, "paused process still burned {} ticks", b1 - b0);
        assert!(process_alive(pid, 0), "suspended, never killed");
        assert_eq!(resume_pids(&paused), paused.len());
        std::thread::sleep(Duration::from_millis(300));
        let c0 = cpu_ticks(pid).unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        let c1 = cpu_ticks(pid).unwrap();
        assert!(c1 - c0 > 2_000_000, "after resume it must run again: {}", c1 - c0);
        let _ = ch.kill();
        let _ = ch.wait();
    }

    #[test]
    fn tree_walks_descendants_and_never_includes_self() {
        let table = [(10, 1), (20, 10), (30, 20), (40, 10), (50, 99), (std::process::id(), 10)];
        assert_eq!(tree_of(10, &table), vec![10, 20, 40, 30]);
        assert!(tree_of(std::process::id(), &table).is_empty());
        assert_eq!(tree_of(4, &[(4, 0), (8, 4)]), vec![8]);
    }

    /// A busy dummy child that is killed when dropped, so a panicking test cannot leak a CPU burner.
    struct Burner(std::process::Child);
    impl std::ops::Deref for Burner {
        type Target = std::process::Child;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
    impl std::ops::DerefMut for Burner {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
    impl Drop for Burner {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn busy_child() -> Burner {
        use std::os::windows::process::CommandExt;
        let ch = std::process::Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", "$e=[Environment]::TickCount+900000; while([Environment]::TickCount -lt $e){ $x=1+1 }"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW: no blue popup
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn dummy");
        // Under host load PowerShell can take many seconds to start its loop; suspending it before
        // it burns CPU makes every later rate check meaningless. Wait until it really is busy.
        let pid = ch.id();
        let mut streak = 0;
        for _ in 0..120 {
            streak = if rate(pid, 400) > 2_000_000 { streak + 1 } else { 0 };
            if streak >= 3 {
                break;
            }
        }
        Burner(ch)
    }

    fn rate(pid: u32, ms: u64) -> u64 {
        let a = cpu_ticks(pid).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(ms));
        cpu_ticks(pid).unwrap() - a
    }

    /// Under parallel test load a running child can be starved for a window: retry before failing.
    fn runs_again(pid: u32) -> bool {
        (0..12).any(|_| rate(pid, 1000) > 1_000_000)
    }

    #[test]
    fn pauser_pause_one_resume_one_and_all() {
        let d = tmp("pauser");
        let (mut c1, mut c2) = (busy_child(), busy_child());
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let ag = |c: &std::process::Child, n: &str| Agent { pid: c.id(), session_id: n.into(), name: n.into(), cwd: String::new(), started_ms: 0 };
        let (a1, a2) = (ag(&c1, "one"), ag(&c2, "two"));
        let mut pz = Pauser::with_state(d.join("st.txt"));
        assert_eq!(pz.pause(std::slice::from_ref(&a1)), 1);
        assert!(pz.is_paused("one") && !pz.is_paused("two"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(rate(c1.id(), 800) < 150_000, "one is paused");
        assert!(runs_again(c2.id()), "two keeps running");
        assert!(d.join("st.txt").exists(), "state file lists suspended pids");
        assert_eq!(pz.pause(&[a1.clone(), a2.clone()]), 1, "already paused one is not suspended twice");
        assert!(pz.resume_session("one"));
        assert!(!pz.resume_session("one"));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(runs_again(c1.id()), "one resumed");
        assert!(rate(c2.id(), 800) < 150_000, "two paused");
        assert_eq!(pz.resume_all(), 1);
        assert!(!d.join("st.txt").exists());
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(runs_again(c2.id()), "two resumed");
        let _ = (c1.kill(), c2.kill(), c1.wait(), c2.wait());
    }

    #[test]
    fn state_file_recovery_resumes_and_ignores_reused_pid() {
        let d = tmp("recover");
        let mut c = busy_child();
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let pid = c.id();
        assert!(suspend_tree(pid).contains(&pid));
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(rate(pid, 600) < 150_000);
        let f = d.join("st.txt");
        std::fs::write(&f, format!("1\n{pid} {}\n{pid} 12345\n", creation_ticks(pid).unwrap())).unwrap();
        assert_eq!(resume_from_state(&f), 1, "wrong creation time (reused pid) is ignored");
        assert!(!f.exists());
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(runs_again(pid), "recovered");
        let _ = (c.kill(), c.wait());
    }

    /// Manual: cargo test --release --bin breakguard -- --ignored live_agents --nocapture
    #[test]
    #[ignore]
    fn live_agents() {
        let r = roots_with(None);
        let v = discover(&r, &process_alive);
        println!("LIVE agents={}", v.len());
        for a in &v {
            println!("LIVE|{}|{}|{}|{}", a.pid, a.session_id, a.name, a.cwd);
        }
    }
}
