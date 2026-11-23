//! Live view of Claude Code sessions on this PC: desktop-app metadata, CLI process files and the
//! transcript tails. Pure std (own tiny JSON reader), no UI code except the repaint hook.
//! Field map (checked on a real transcript): see jobs/sessions-fieldmap.md in the ralp run dir.
#![allow(dead_code)]
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// File name of a ralp run's box list.
pub(crate) const BOXFILE: &str = concat!("CHECK", "LIST.md");

// ------------------------------------------------------------------ minimal JSON

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(s: &str) -> Option<Json> {
        // PowerShell Set-Content -Encoding UTF8 writes a BOM; Claude files may carry one too
        let s = s.trim_start_matches('\u{feff}');
        let b = s.as_bytes();
        let mut i = 0;
        let v = value(b, &mut i, 0)?;
        ws(b, &mut i);
        (i == b.len()).then_some(v)
    }
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }
    pub fn num(&self) -> Option<f64> {
        if let Json::Num(n) = self { Some(*n) } else { None }
    }
    pub fn bool(&self) -> Option<bool> {
        if let Json::Bool(b) = self { Some(*b) } else { None }
    }
    pub fn arr(&self) -> &[Json] {
        if let Json::Arr(a) = self { a } else { &[] }
    }
    pub fn gs(&self, k: &str) -> Option<&str> {
        self.get(k).and_then(Json::str)
    }
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize, depth: u32) -> Option<Json> {
    if depth > 64 {
        return None;
    }
    ws(b, i);
    match *b.get(*i)? {
        b'{' => {
            *i += 1;
            let mut v = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Some(Json::Obj(v));
            }
            loop {
                ws(b, i);
                let k = string(b, i)?;
                ws(b, i);
                if *b.get(*i)? != b':' {
                    return None;
                }
                *i += 1;
                v.push((k, value(b, i, depth + 1)?));
                ws(b, i);
                match *b.get(*i)? {
                    b',' => *i += 1,
                    b'}' => {
                        *i += 1;
                        return Some(Json::Obj(v));
                    }
                    _ => return None,
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut v = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Some(Json::Arr(v));
            }
            loop {
                v.push(value(b, i, depth + 1)?);
                ws(b, i);
                match *b.get(*i)? {
                    b',' => *i += 1,
                    b']' => {
                        *i += 1;
                        return Some(Json::Arr(v));
                    }
                    _ => return None,
                }
            }
        }
        b'"' => string(b, i).map(Json::Str),
        b't' if b[*i..].starts_with(b"true") => {
            *i += 4;
            Some(Json::Bool(true))
        }
        b'f' if b[*i..].starts_with(b"false") => {
            *i += 5;
            Some(Json::Bool(false))
        }
        b'n' if b[*i..].starts_with(b"null") => {
            *i += 4;
            Some(Json::Null)
        }
        _ => {
            let st = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            std::str::from_utf8(&b[st..*i]).ok()?.parse().ok().map(Json::Num)
        }
    }
}

fn hex4(b: &[u8], i: usize) -> Option<u32> {
    u32::from_str_radix(std::str::from_utf8(b.get(i..i + 4)?).ok()?, 16).ok()
}

fn string(b: &[u8], i: &mut usize) -> Option<String> {
    if *b.get(*i)? != b'"' {
        return None;
    }
    *i += 1;
    let mut out: Vec<u8> = Vec::new();
    loop {
        let c = *b.get(*i)?;
        *i += 1;
        match c {
            b'"' => return String::from_utf8(out).ok(),
            b'\\' => {
                let e = *b.get(*i)?;
                *i += 1;
                let ch = match e {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'/' => '/',
                    b'\\' => '\\',
                    b'"' => '"',
                    b'u' => {
                        let mut cp = hex4(b, *i)?;
                        *i += 4;
                        if (0xD800..0xDC00).contains(&cp) && b.get(*i) == Some(&b'\\') && b.get(*i + 1) == Some(&b'u') {
                            let lo = hex4(b, *i + 2)?;
                            if (0xDC00..0xE000).contains(&lo) {
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                *i += 6;
                            }
                        }
                        char::from_u32(cp).unwrap_or('\u{FFFD}')
                    }
                    _ => return None,
                };
                let mut buf = [0u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            _ => out.push(c),
        }
    }
}

// ------------------------------------------------------------------ model

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Env {
    Desktop,
    Cli,
    Ide,
    Cloud,
    Remote,
    Unknown,
}

impl Env {
    pub fn label(self) -> &'static str {
        match self {
            Env::Desktop => "Desktop",
            Env::Cli => "CLI",
            Env::Ide => "IDE",
            Env::Cloud => "Cloud",
            Env::Remote => "Remote",
            Env::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Running,
    WaitingInput,
    Error,
    Idle,
    Finished,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Running => "Running",
            Status::WaitingInput => "Needs input",
            Status::Error => "Error",
            Status::Idle => "Idle",
            Status::Finished => "Finished",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Status::Running => 0,
            Status::WaitingInput => 1,
            Status::Error => 2,
            Status::Idle => 3,
            Status::Finished => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    /// Exactly the title the app shows (never trimmed or renamed).
    pub name: String,
    pub cwd: String,
    pub env: Env,
    pub status: Status,
    pub archived: bool,
    pub created_ms: i64,
    pub last_active_ms: i64,
    pub activity: String,
    pub progress: Option<(u32, u32)>,
    pub note: Option<String>,
    /// Last few activity lines (oldest first) for the per-session log tail.
    pub tail: Vec<String>,
}

/// Lets at most one progress event per `min_ms` through for each session id.
pub struct ProgressThrottle {
    min_ms: i64,
    last: HashMap<String, i64>,
}

impl ProgressThrottle {
    pub fn new(min_ms: i64) -> Self {
        Self { min_ms, last: HashMap::new() }
    }
    /// true = deliver this event now; false = drop it (too soon after the last delivered one).
    pub fn allow(&mut self, id: &str, now_ms: i64) -> bool {
        match self.last.get(id) {
            Some(&t) if now_ms - t < self.min_ms => false,
            _ => {
                self.last.insert(id.to_string(), now_ms);
                true
            }
        }
    }
}

/// Progress view of a session: percent, current step label, started-at, updated-at (unix ms).
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressInfo {
    pub percent: Option<u8>,
    pub step_label: String,
    pub started_ms: i64,
    pub updated_ms: i64,
}

impl SessionInfo {
    pub fn progress_info(&self) -> ProgressInfo {
        let percent = self.progress.and_then(|(d, t)| if t == 0 { None } else { Some((d.min(t) as u64 * 100 / t as u64) as u8) });
        ProgressInfo { percent, step_label: self.activity.clone(), started_ms: self.created_ms, updated_ms: self.last_active_ms }
    }
}

/// Why cloud / remote sessions have no live data here.
pub fn cloud_note(env: Env) -> Option<&'static str> {
    match env {
        Env::Cloud => Some("Cloud session: not live from this PC (last known state from the app cache)"),
        Env::Remote => Some("Remote session: not live from this PC (last known state from the app cache)"),
        _ => None,
    }
}

// ------------------------------------------------------------------ text helpers

/// Cut to at most `max` characters (never inside a character), ending with an ellipsis when cut.
pub fn truncate_chars(s: &str, max: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    let mut t: String = one.chars().take(max.saturating_sub(1)).collect();
    t.push('\u{2026}');
    t
}

/// "5s ago", "3m ago", "2h ago", "4d ago".
pub fn age_text(secs: i64) -> String {
    let s = secs.max(0);
    match s {
        0..=59 => format!("{s}s ago"),
        60..=3599 => format!("{}m ago", s / 60),
        3600..=86_399 => format!("{}h ago", s / 3600),
        _ => format!("{}d ago", s / 86_400),
    }
}

/// "12s", "5m 03s", "1h 05m".
pub fn elapsed_text(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    }
}

// ------------------------------------------------------------------ incremental tail reader

/// First read looks only at the last `TAIL_CAP` bytes of a file.
pub const TAIL_CAP: u64 = 256 * 1024;

#[derive(Debug)]
pub struct TailReader {
    path: PathBuf,
    pos: u64,
    carry: Vec<u8>,
    started: bool,
}

impl TailReader {
    pub fn new(path: PathBuf) -> Self {
        TailReader { path, pos: 0, carry: Vec::new(), started: false }
    }

    /// New complete lines since the last call. A partial last line waits for its newline; a file
    /// that shrank is read again from its tail.
    pub fn read_new(&mut self) -> Vec<String> {
        let Ok(mut f) = std::fs::File::open(&self.path) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.pos {
            self.started = false;
            self.carry.clear();
        }
        let mut skip_first = false;
        if !self.started {
            self.started = true;
            self.carry.clear();
            if len > TAIL_CAP {
                self.pos = len - TAIL_CAP;
                skip_first = true;
            } else {
                self.pos = 0;
            }
        }
        if len == self.pos || f.seek(SeekFrom::Start(self.pos)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        let _ = f.take(len - self.pos).read_to_end(&mut buf);
        self.pos += buf.len() as u64;
        self.carry.extend_from_slice(&buf);
        let mut lines = Vec::new();
        while let Some(n) = self.carry.iter().position(|&c| c == b'\n') {
            let line: Vec<u8> = self.carry.drain(..=n).collect();
            if skip_first {
                skip_first = false; // first line of a cut read is probably partial
                continue;
            }
            let t = String::from_utf8_lossy(&line[..line.len() - 1]).trim_end_matches('\r').to_string();
            if !t.is_empty() {
                lines.push(t);
            }
        }
        if skip_first {
            self.carry.clear();
        }
        lines
    }
}

// ------------------------------------------------------------------ transcript analysis

/// Record types that carry no conversation state.
const NOISE: [&str; 14] = [
    "attachment", "api-request", "api-request-blob", "api-request-shape", "last-prompt", "custom-title",
    "agent-name", "atis-latch", "bridge-session", "queue-operation", "file-history-snapshot",
    "file-history-delta", "tools-used", "permission-mode",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub status: Status,
    pub activity: String,
    pub progress: Option<(u32, u32)>,
}

fn tool_summary(name: &str, input: Option<&Json>) -> String {
    let get = |k: &str| input.and_then(|i| i.gs(k)).map(str::to_string);
    let detail = get("description")
        .or_else(|| get("command"))
        .or_else(|| get("file_path"))
        .or_else(|| get("pattern"))
        .or_else(|| get("query"))
        .or_else(|| get("url"))
        .unwrap_or_default();
    if detail.is_empty() { name.to_string() } else { format!("{name}: {detail}") }
}

/// Done/total from a plan or todo tool call (`update_plan` items, `TodoWrite` todos).
fn plan_progress(name: &str, input: &Json) -> Option<(u32, u32)> {
    let list = match name {
        n if n.ends_with("update_plan") => input.get("items")?,
        "TodoWrite" => input.get("todos")?,
        _ => return None,
    };
    let items = list.arr();
    if items.is_empty() {
        return None;
    }
    let done = items.iter().filter(|i| i.gs("status") == Some("completed")).count() as u32;
    Some((done, items.len() as u32))
}

/// Done/total from a markdown box list (`- [x]` / `- [ ]` lines).
pub fn checklist_progress(text: &str) -> Option<(u32, u32)> {
    let (mut done, mut total) = (0, 0);
    for l in text.lines() {
        let t = l.trim_start();
        let t = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).unwrap_or(t);
        if t.starts_with("[x]") || t.starts_with("[X]") {
            done += 1;
            total += 1;
        } else if t.starts_with("[ ]") {
            total += 1;
        }
    }
    (total > 0).then_some((done, total))
}

/// Path of a ralp run's box list mentioned in text, if any.
pub fn find_checklist_path(text: &str) -> Option<String> {
    let end = text.rfind(BOXFILE)? + BOXFILE.len();
    let head = &text[..end];
    let start = head.rfind(['"', '\'', ' ', '\n', '`', '(']).map_or(0, |p| p + 1);
    let p = head[start..].replace("\\\\", "/").replace('\\', "/");
    (p.contains('/') && p.len() > BOXFILE.len()).then_some(p)
}

/// Status, activity line and progress from the newest transcript lines.
/// `age_secs` = time since the file last changed.
/// Last `n` activity summaries (tool calls and assistant text), oldest first.
pub fn recent_activity(lines: &[String], n: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in lines.iter().rev() {
        let Some(j) = Json::parse(l) else { continue };
        if j.gs("type") != Some("assistant") {
            continue;
        }
        let Some(Json::Arr(blocks)) = j.get("message").and_then(|m| m.get("content")) else { continue };
        for b in blocks.iter().rev() {
            let t = match b.gs("type") {
                Some("tool_use") => truncate_chars(&tool_summary(b.gs("name").unwrap_or("tool"), b.get("input")), 140),
                Some("text") => truncate_chars(b.gs("text").unwrap_or(""), 140),
                _ => continue,
            };
            if !t.is_empty() && out.len() < n {
                out.push(t);
            }
        }
        if out.len() >= n {
            break;
        }
    }
    out.reverse();
    out
}

pub fn analyze(lines: &[String], age_secs: i64, archived: bool) -> Analysis {
    let mut activity = String::new();
    let mut progress = None;
    let mut last_state: Option<(bool, bool)> = None; // (ended_turn, asks_user)
    let mut errored = false;
    let mut seen_state = false;
    for l in lines.iter().rev() {
        let Some(j) = Json::parse(l) else { continue };
        let ty = j.gs("type").unwrap_or("");
        let sub = j.gs("subtype").unwrap_or("");
        if NOISE.contains(&ty) || (ty == "system" && sub != "api_error") {
            continue;
        }
        let content = j.get("message").and_then(|m| m.get("content"));
        if let (Some(Json::Arr(blocks)), "assistant") = (content, ty) {
            for b in blocks.iter().rev() {
                match b.gs("type") {
                    Some("tool_use") => {
                        let name = b.gs("name").unwrap_or("tool");
                        if progress.is_none() {
                            progress = b.get("input").and_then(|i| plan_progress(name, i));
                        }
                        if activity.is_empty() {
                            activity = truncate_chars(&tool_summary(name, b.get("input")), 140);
                        }
                    }
                    Some("text") if activity.is_empty() => {
                        activity = truncate_chars(b.gs("text").unwrap_or(""), 140);
                    }
                    _ => {}
                }
            }
        }
        if !seen_state {
            seen_state = true;
            if l.contains("\"isApiErrorMessage\":true") || sub == "api_error" {
                errored = true;
            }
            let ended = ty == "assistant" && j.get("message").and_then(|m| m.gs("stop_reason")) == Some("end_turn");
            let asks = matches!(content, Some(Json::Arr(bl)) if bl.iter().any(|b| {
                b.gs("type") == Some("tool_use") && matches!(b.gs("name"), Some("AskUserQuestion" | "ExitPlanMode"))
            }));
            last_state = Some((ended, asks));
        }
        if progress.is_some() && !activity.is_empty() {
            break;
        }
    }
    let status = if archived {
        Status::Finished
    } else if errored {
        Status::Error
    } else {
        match last_state {
            None => Status::Idle,
            Some((_, true)) if age_secs >= 3 => Status::WaitingInput,
            Some((true, _)) => {
                if age_secs < 2 * 3600 { Status::WaitingInput } else { Status::Idle }
            }
            Some(_) => {
                if age_secs < 120 { Status::Running } else { Status::Idle }
            }
        }
    };
    Analysis { status, activity, progress }
}

// ------------------------------------------------------------------ scanning

#[derive(Clone, Debug)]
pub struct Roots {
    /// `<appdata>/Claude/claude-code-sessions`
    pub desktop: PathBuf,
    /// ~/.claude/projects
    pub projects: PathBuf,
    /// ~/.claude/sessions (one `<pid>.json` per CLI process)
    pub cli: PathBuf,
}

/// Roots for this PC, or all under `over` (the BREAKGUARD_SESSIONS_DIR fixture layout).
pub fn roots_with(over: Option<&Path>) -> Roots {
    if let Some(d) = over {
        return Roots { desktop: d.join("claude-code-sessions"), projects: d.join("projects"), cli: d.join("sessions") };
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    let app = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| home.join("AppData").join("Roaming"));
    Roots { desktop: app.join("Claude").join("claude-code-sessions"), projects: home.join(".claude").join("projects"), cli: home.join(".claude").join("sessions") }
}

pub fn roots() -> Roots {
    let mut over = std::env::var_os("BREAKGUARD_SESSIONS_DIR").map(PathBuf::from);
    if over.is_none() && crate::sys::is_mock() {
        // mock runs (tests, screenshots) never read the live session folders
        over = Some(crate::engine::data_dir().join("no-sessions"));
    }
    roots_with(over.as_deref())
}

fn mtime_ms(p: &Path) -> Option<i64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(m.duration_since(UNIX_EPOCH).ok()?.as_millis() as i64)
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn json_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 3 {
                json_files(&p, depth + 1, out);
            }
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// Environment of a desktop-app session from its metadata.
pub fn desktop_env(j: &Json) -> Env {
    let has = |k: &str| j.get(k).is_some_and(|v| !matches!(v, Json::Null | Json::Bool(false)) && !v.str().is_some_and(str::is_empty));
    if has("cloudSessionId") || j.gs("environmentKind") == Some("cloud") || j.gs("target") == Some("cloud") {
        Env::Cloud
    } else if has("sshHost") || has("remoteHost") || j.gs("target") == Some("ssh") {
        Env::Remote
    } else if j.gs("sessionId").is_some_and(|s| s.starts_with("local_")) {
        Env::Desktop
    } else {
        Env::Unknown
    }
}

/// Environment of a CLI process file from its `entrypoint`.
pub fn cli_env(entrypoint: &str) -> Env {
    let e = entrypoint.to_ascii_lowercase();
    if e.contains("vscode") || e.contains("jetbrains") || e.contains("ide") || e.contains("cursor") || e.contains("zed") {
        Env::Ide
    } else if e == "cli" || e == "sdk-cli" || e.contains("terminal") {
        Env::Cli
    } else if e.contains("desktop") {
        Env::Desktop
    } else {
        Env::Unknown
    }
}

struct Tail {
    reader: TailReader,
    lines: VecDeque<String>,
    /// Bumped whenever new lines arrive; the memo is reused while it is unchanged.
    ver: u64,
    memo: Option<Memo>,
    /// Transcript (modified time, size) at the last read.
    read_mt: Option<(i64, u64)>,
}

/// Parsed result of a transcript tail. `analyze` only depends on the age through its thresholds,
/// so the memo is keyed by the line version, the archived flag and the age class.
struct Memo {
    key: (u64, bool, AgeClass),
    a: Analysis,
    tail: Vec<String>,
    checklist: Option<String>,
}

type AgeClass = (bool, bool, bool);

/// The age thresholds `analyze` compares against (3 s, 120 s, 2 h).
pub fn age_class(age_secs: i64) -> AgeClass {
    (age_secs >= 3, age_secs < 120, age_secs < 2 * 3600)
}

pub struct Scanner {
    roots: Roots,
    tails: HashMap<PathBuf, Tail>,
    /// Parsed desktop session files by modified time; shared, so a scan never deep-copies them.
    metas: HashMap<PathBuf, (i64, std::rc::Rc<Json>)>,
    transcript_index: (Instant, HashMap<String, PathBuf>),
    checklists: HashMap<PathBuf, (i64, Option<(u32, u32)>)>,
    /// Ids looked up and not found since the last full index rebuild (no rebuild per second for them).
    missed: HashSet<String>,
}

const KEEP_LINES: usize = 300;

impl Scanner {
    pub fn new(roots: Roots) -> Self {
        Scanner {
            roots,
            tails: HashMap::new(),
            metas: HashMap::new(),
            transcript_index: (Instant::now() - Duration::from_secs(3600), HashMap::new()),
            checklists: HashMap::new(),
            missed: HashSet::new(),
        }
    }

    fn transcript(&mut self, cli_id: &str) -> Option<PathBuf> {
        let age = self.transcript_index.0.elapsed();
        if age > Duration::from_secs(5) || (!self.transcript_index.1.contains_key(cli_id) && !self.missed.contains(cli_id) && age > Duration::from_secs(1)) {
            if age > Duration::from_secs(5) {
                self.missed.clear();
            }
            let mut m = HashMap::new();
            if let Ok(rd) = std::fs::read_dir(&self.roots.projects) {
                for d in rd.flatten() {
                    if let Ok(files) = std::fs::read_dir(d.path()) {
                        for f in files.flatten() {
                            let p = f.path();
                            if p.extension().is_some_and(|x| x == "jsonl") {
                                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                                    m.insert(stem.to_string(), p);
                                }
                            }
                        }
                    }
                }
            }
            self.transcript_index = (Instant::now(), m);
        }
        let hit = self.transcript_index.1.get(cli_id).cloned();
        if hit.is_none() {
            self.missed.insert(cli_id.to_string());
        }
        hit
    }

    /// Analysis, recent activity, checklist progress and last write time of one transcript. New
    /// bytes are read every call, but the lines are parsed again only when they or the age class
    /// changed (re-parsing every session's 300 lines each second cost ~0.6 s CPU per second).
    fn digest(&mut self, path: &Path, now: i64, last_active: i64, archived: bool) -> (Analysis, Vec<String>, Option<(u32, u32)>, i64) {
        let mt = mtime_ms(path).unwrap_or(0);
        let last = last_active.max(mt);
        let t = self.tails.entry(path.to_path_buf()).or_insert_with(|| Tail { reader: TailReader::new(path.to_path_buf()), lines: VecDeque::new(), ver: 0, memo: None, read_mt: None });
        // an unchanged modified time means no new bytes: skip opening the file (~270 transcripts a second)
        // (size too: two writes in the same millisecond keep the time but never the size)
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let fresh = if t.read_mt == Some((mt, len)) && mt != 0 { Vec::new() } else { t.reader.read_new() };
        t.read_mt = Some((mt, len));
        if !fresh.is_empty() {
            t.ver += 1;
        }
        for l in fresh {
            t.lines.push_back(l);
            if t.lines.len() > KEEP_LINES {
                t.lines.pop_front();
            }
        }
        let key = (t.ver, archived, age_class(((now - last) / 1000).max(0)));
        if t.memo.as_ref().map(|m| m.key) != Some(key) {
            let lines: Vec<String> = t.lines.iter().cloned().collect();
            let a = analyze(&lines, ((now - last) / 1000).max(0), archived);
            let tail = recent_activity(&lines, 6);
            let recent: String = lines.iter().rev().take(80).cloned().collect::<Vec<_>>().join("\n");
            t.memo = Some(Memo { key, a, tail, checklist: find_checklist_path(&recent) });
        }
        let m = t.memo.as_ref().expect("memo set above");
        let (a, tail, cl) = (m.a.clone(), m.tail.clone(), m.checklist.clone());
        let c = cl.and_then(|p| self.checklist_at(PathBuf::from(p)));
        (a, tail, c, last)
    }

    fn checklist_at(&mut self, path: PathBuf) -> Option<(u32, u32)> {
        let mt = mtime_ms(&path)?;
        if let Some((m, p)) = self.checklists.get(&path) {
            if *m == mt {
                return *p;
            }
        }
        let p = std::fs::read_to_string(&path).ok().and_then(|t| checklist_progress(&t));
        self.checklists.insert(path, (mt, p));
        p
    }

    /// Everything known right now. `ok` is false when no source could be read at all.
    pub fn scan(&mut self, now: i64) -> (Vec<SessionInfo>, bool) {
        let mut out: Vec<SessionInfo> = Vec::new();
        let mut files = Vec::new();
        json_files(&self.roots.desktop, 0, &mut files);
        let mut seen_cli: HashSet<String> = HashSet::new();
        for p in files {
            let mt = mtime_ms(&p).unwrap_or(0);
            let fresh = self.metas.get(&p).is_some_and(|(m, _)| *m == mt);
            if !fresh {
                match std::fs::read_to_string(&p).ok().and_then(|t| Json::parse(&t)) {
                    Some(j) => {
                        self.metas.insert(p.clone(), (mt, std::rc::Rc::new(j)));
                    }
                    None => continue,
                }
            }
            let j = std::rc::Rc::clone(&self.metas[&p].1);
            let Some(id) = j.gs("sessionId") else { continue };
            let cli_id = j.gs("cliSessionId").unwrap_or("").to_string();
            let env = desktop_env(&j);
            let archived = j.get("isArchived").and_then(Json::bool).unwrap_or(false);
            let meta_active = j.get("lastActivityAt").and_then(Json::num).unwrap_or(0.0) as i64;
            let (mut status, mut activity, mut progress, mut last_active) = (if archived { Status::Finished } else { Status::Idle }, String::new(), None, meta_active);
            let mut tail: Vec<String> = Vec::new();
            if matches!(env, Env::Desktop | Env::Cli | Env::Ide) && !cli_id.is_empty() {
                seen_cli.insert(cli_id.clone());
                if let Some(tp) = self.transcript(&cli_id) {
                    let (a, t, c, last) = self.digest(&tp, now, last_active, archived);
                    last_active = last;
                    status = a.status;
                    activity = a.activity;
                    progress = c.or(a.progress);
                    tail = t;
                }
            }
            out.push(SessionInfo {
                id: id.to_string(),
                name: j.gs("title").unwrap_or("").to_string(),
                cwd: j.gs("cwd").unwrap_or("").to_string(),
                env,
                status,
                archived,
                created_ms: j.get("createdAt").and_then(Json::num).unwrap_or(0.0) as i64,
                last_active_ms: last_active,
                activity,
                progress,
                note: cloud_note(env).map(str::to_string),
                tail,
            });
        }
        // CLI / IDE processes that the desktop app does not know
        let mut cli_files = Vec::new();
        json_files(&self.roots.cli, 2, &mut cli_files);
        for p in cli_files {
            let Some(j) = std::fs::read_to_string(&p).ok().and_then(|t| Json::parse(&t)) else { continue };
            let (Some(sid), ep) = (j.gs("sessionId"), j.gs("entrypoint").unwrap_or("")) else { continue };
            let env = cli_env(ep);
            if env == Env::Desktop || j.gs("hostSessionId").is_some() || seen_cli.contains(sid) {
                continue;
            }
            let started = j.get("startedAt").and_then(Json::num).unwrap_or(0.0) as i64;
            let (mut status, mut activity, mut progress, mut last) = (Status::Idle, String::new(), None, started);
            let mut tail: Vec<String> = Vec::new();
            if let Some(tp) = self.transcript(sid) {
                let (a, t, c, l) = self.digest(&tp, now, last, false);
                last = l;
                status = a.status;
                activity = a.activity;
                progress = c.or(a.progress);
                tail = t;
            }
            let cwd = j.gs("cwd").unwrap_or("").to_string();
            let name = Path::new(&cwd).file_name().and_then(|n| n.to_str()).unwrap_or("session").to_string();
            out.push(SessionInfo { id: sid.to_string(), name, cwd, env, status, archived: false, created_ms: started, last_active_ms: last, activity, progress, note: None, tail });
        }
        let ok = self.roots.desktop.is_dir() || self.roots.cli.is_dir();
        (out, ok)
    }
}

// ------------------------------------------------------------------ snapshots

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub sessions: Vec<SessionInfo>,
    pub taken: Instant,
    pub ok: bool,
    /// Running Claude sessions by the same discovery the Live tab uses (status bar count).
    pub live: usize,
    /// Throttled progress events since the previous snapshot (same channel, no second pipe).
    pub progress: Vec<(String, ProgressInfo)>,
}

/// A snapshot older than this, or a failed refresh, is shown as "Not live".
pub const STALE_AFTER: Duration = Duration::from_secs(5);

pub fn is_stale(s: &Snapshot, now: Instant) -> bool {
    !s.ok || now.saturating_duration_since(s.taken) > STALE_AFTER
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Finished(String, String),
    NeedsInput(String, String),
}

/// Sessions that changed to Finished or WaitingInput between two snapshots.
pub fn diff(old: &[SessionInfo], new: &[SessionInfo]) -> Vec<Event> {
    let mut ev = Vec::new();
    for n in new {
        let Some(o) = old.iter().find(|o| o.id == n.id) else { continue };
        if o.status == n.status {
            continue;
        }
        match n.status {
            Status::Finished => ev.push(Event::Finished(n.id.clone(), n.name.clone())),
            Status::WaitingInput => ev.push(Event::NeedsInput(n.id.clone(), n.name.clone())),
            _ => {}
        }
    }
    ev
}

/// Snapshot carrying the throttled progress events computed against the previous scan.
pub fn build_snapshot(prev: &[SessionInfo], sessions: Vec<SessionInfo>, ok: bool, live: usize, th: &mut ProgressThrottle, now_ms: i64) -> Snapshot {
    let progress = progress_events(prev, &sessions, th, now_ms);
    Snapshot { sessions, taken: Instant::now(), ok, live, progress }
}

/// Poll every second on its own thread; the UI only receives finished snapshots.
pub fn spawn(ctx: eframe::egui::Context) -> Receiver<Snapshot> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let live_roots = roots();
        let mut sc = Scanner::new(roots());
        let mut th = ProgressThrottle::new(250);
        let mut prev: Vec<SessionInfo> = Vec::new();
        loop {
            let t0 = Instant::now();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sc.scan(now_ms())));
            let live = std::panic::catch_unwind(|| crate::agents::count_live(&live_roots, &crate::agents::process_alive)).unwrap_or(0);
            let snap = match r {
                Ok((sessions, ok)) => build_snapshot(&prev, sessions, ok, live, &mut th, now_ms()),
                Err(_) => Snapshot { sessions: Vec::new(), taken: Instant::now(), ok: false, live, progress: Vec::new() },
            };
            prev = snap.sessions.clone();
            if tx.send(snap).is_err() {
                return;
            }
            ctx.request_repaint();
            std::thread::sleep(Duration::from_secs(1).saturating_sub(t0.elapsed()));
        }
    });
    rx
}

// ------------------------------------------------------------------ sort / filter / aggregate

/// Pinned first, then running, waiting, error, idle, finished; newest activity first inside a group.
pub fn sort_sessions(v: &mut [SessionInfo], pinned: &HashSet<String>) {
    v.sort_by(|a, b| {
        let ka = (!pinned.contains(&a.id), a.status.rank(), std::cmp::Reverse(a.last_active_ms));
        let kb = (!pinned.contains(&b.id), b.status.rank(), std::cmp::Reverse(b.last_active_ms));
        ka.cmp(&kb)
    });
}

/// Progress events for every session whose step label or progress changed between two scans,
/// already throttled (one per `ProgressThrottle` window per session).
pub fn progress_events(prev: &[SessionInfo], next: &[SessionInfo], th: &mut ProgressThrottle, now_ms: i64) -> Vec<(String, ProgressInfo)> {
    let old: HashMap<&str, &SessionInfo> = prev.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut out = Vec::new();
    for s in next {
        let changed = old.get(s.id.as_str()).map_or(true, |o| o.activity != s.activity || o.progress != s.progress);
        if changed && th.allow(&s.id, now_ms) {
            out.push((s.id.clone(), s.progress_info()));
        }
    }
    out
}

/// Reactive-style store of the latest progress per session id; `apply` reports which ids changed.
#[derive(Default)]
pub struct ProgressStore {
    pub by_id: HashMap<String, ProgressInfo>,
}

impl ProgressStore {
    pub fn apply(&mut self, ev: &[(String, ProgressInfo)]) -> Vec<String> {
        let mut changed = Vec::new();
        for (id, p) in ev {
            if self.by_id.get(id) != Some(p) {
                self.by_id.insert(id.clone(), p.clone());
                changed.push(id.clone());
            }
        }
        changed
    }
}

/// Finished-tab card line: outcome, total duration, finished-at (relative) and, for a failure, the error summary.
pub fn finished_summary(s: &SessionInfo, now_ms: i64) -> String {
    let outcome = match s.status.phase() {
        Phase::Finished => "Finished",
        Phase::Failed => "Failed",
        Phase::Cancelled => "Cancelled",
        Phase::Paused => "Stopped (idle)",
        Phase::Running => "Running",
    };
    let dur = elapsed_text((s.last_active_ms - s.created_ms) / 1000);
    let at = age_text((now_ms - s.last_active_ms) / 1000);
    let mut out = format!("{outcome}  |  took {dur}  |  finished {at}");
    if s.status == Status::Error {
        let why = s.note.as_deref().filter(|n| !n.is_empty()).unwrap_or(s.activity.as_str());
        if !why.is_empty() {
            out.push_str("  |  ");
            out.push_str(why);
        }
    }
    out
}

/// Tab order: pinned first, then newest activity first (Running = most recent activity, Finished = finished-at,
/// which is the last activity of an ended session); ties by id so the order is stable.
pub fn sort_for_tab(v: &mut [SessionInfo], pinned: &HashSet<String>) {
    v.sort_by(|a, b| (!pinned.contains(&a.id), std::cmp::Reverse(a.last_active_ms), &a.id).cmp(&(!pinned.contains(&b.id), std::cmp::Reverse(b.last_active_ms), &b.id)));
}

/// Search text (name, cwd, activity; case-insensitive) and status chips (empty set = all).
pub fn matches(s: &SessionInfo, query: &str, statuses: &HashSet<Status>) -> bool {
    if !statuses.is_empty() && !statuses.contains(&s.status) {
        return false;
    }
    let q = query.trim().to_lowercase();
    q.is_empty() || [&s.name, &s.cwd, &s.activity].iter().any(|f| f.to_lowercase().contains(&q))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: u32,
    pub running: u32,
    pub waiting: u32,
    pub done: u32,
    pub idle: u32,
    pub error: u32,
    /// Sum of done and of total over sessions that report progress.
    pub progress: (u32, u32),
}

pub fn summarize(v: &[SessionInfo]) -> Summary {
    let mut s = Summary::default();
    for x in v {
        s.total += 1;
        match x.status {
            Status::Running => s.running += 1,
            Status::WaitingInput => s.waiting += 1,
            Status::Finished => s.done += 1,
            Status::Idle => s.idle += 1,
            Status::Error => s.error += 1,
        }
        if let Some((d, t)) = x.progress {
            s.progress.0 += d;
            s.progress.1 += t;
        }
    }
    s
}

/// Header tile counts that agree with the tabs: (running = Running + needs input, finished = every other session).
pub fn tile_counts(s: &Summary) -> (u32, u32) {
    let running = s.running + s.waiting;
    (running, s.total - running)
}

/// Canonical lifecycle phase every view uses (one source of truth for Running vs not).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Running,
    Paused,
    Finished,
    Failed,
    Cancelled,
}

impl Phase {
    pub fn is_running(self) -> bool {
        self == Phase::Running
    }
}

impl Status {
    /// Needs-input still counts as running (the session is alive and working toward an answer); Idle = Paused.
    pub fn phase(self) -> Phase {
        match self {
            Status::Running | Status::WaitingInput => Phase::Running,
            Status::Idle => Phase::Paused,
            Status::Finished => Phase::Finished,
            Status::Error => Phase::Failed,
        }
    }
}

pub fn is_running(s: &SessionInfo) -> bool {
    s.status.phase().is_running()
}

pub fn running_sessions(v: &[SessionInfo]) -> Vec<&SessionInfo> {
    v.iter().filter(|s| is_running(s)).collect()
}

pub fn finished_sessions(v: &[SessionInfo]) -> Vec<&SessionInfo> {
    v.iter().filter(|s| !is_running(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn mk(id: &str, st: Status) -> SessionInfo {
        SessionInfo { id: id.into(), name: id.into(), cwd: String::new(), env: Env::Unknown, status: st, archived: false, created_ms: 0, last_active_ms: 0, activity: String::new(), progress: None, note: None, tail: vec![] }
    }
    const ALL_ST: [Status; 5] = [Status::Running, Status::WaitingInput, Status::Error, Status::Idle, Status::Finished];

    #[test]
    fn progress_info_exposes_all_four_fields() {
        let mut s = mk("p", Status::Running);
        s.progress = Some((3, 4));
        s.activity = "Bash: build".into();
        s.created_ms = 10;
        s.last_active_ms = 20;
        assert_eq!(s.progress_info(), ProgressInfo { percent: Some(75), step_label: "Bash: build".into(), started_ms: 10, updated_ms: 20 });
        s.progress = Some((0, 0));
        assert_eq!(s.progress_info().percent, None);
    }

    #[test]
    fn throttle_100_events_in_1s_delivers_under_5() {
        let mut th = ProgressThrottle::new(250);
        let delivered = (0..100).filter(|i| th.allow("a", i * 10)).count();
        assert!(delivered < 5 && delivered >= 3, "{delivered}");
        assert!(th.allow("b", 5), "other id has its own budget");
    }

    #[test]
    fn tab_sort_newest_first_pinned_on_top() {
        let mut v: Vec<SessionInfo> = [("a", 10), ("b", 50), ("c", 30), ("d", 50), ("e", 5)].iter().map(|(i, t)| { let mut s = mk(i, Status::Finished); s.last_active_ms = *t; s }).collect();
        let pin: HashSet<String> = ["e".to_string()].into();
        sort_for_tab(&mut v, &pin);
        assert_eq!(v.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["e", "b", "d", "c", "a"]);
    }

    #[test]
    fn finished_summary_has_outcome_duration_time_and_error() {
        let mut s = mk("f", Status::Error);
        s.created_ms = 0;
        s.last_active_ms = 125_000;
        s.activity = "cargo build failed".into();
        let t = finished_summary(&s, 185_000);
        assert_eq!(t, "Failed  |  took 2m 05s  |  finished 1m ago  |  cargo build failed");
        s.status = Status::Finished;
        assert_eq!(finished_summary(&s, 185_000), "Finished  |  took 2m 05s  |  finished 1m ago");
    }

    #[test]
    fn runner_emits_event_per_step_change_and_store_updates_only_that_id() {
        let mut th = ProgressThrottle::new(250);
        let mut store = ProgressStore::default();
        let mut prev = vec![mk("a", Status::Running), mk("b", Status::Running)];
        let mut events = Vec::new();
        for (i, step) in ["one", "two", "three"].iter().enumerate() {
            let mut next = prev.clone();
            next[0].activity = step.to_string();
            let ev = progress_events(&prev, &next, &mut th, 1000 * (i as i64 + 1));
            events.extend(ev.clone());
            let changed = store.apply(&ev);
            assert_eq!(changed, ["a"], "only a re-renders (b untouched)");
            prev = next;
        }
        assert!(events.len() >= 3, "{}", events.len());
        assert!(events.iter().all(|(id, _)| id == "a"));
        // unchanged scan emits nothing
        assert!(progress_events(&prev, &prev.clone(), &mut th, 9000).is_empty());
    }

    #[test]
    fn progress_events_arrive_over_the_single_channel() {
        let (tx, rx) = channel::<Snapshot>();
        let mut th = ProgressThrottle::new(250);
        let a0 = mk("a", Status::Running);
        let mut a1 = a0.clone();
        a1.activity = "step two".into();
        a1.progress = Some((2, 4));
        let first = build_snapshot(&[], vec![a0.clone()], true, 1, &mut th, 0);
        tx.send(first).unwrap();
        let second = build_snapshot(&[a0], vec![a1], true, 1, &mut th, 1000);
        tx.send(second).unwrap();
        let got: Vec<Snapshot> = rx.try_iter().collect();
        assert_eq!(got.len(), 2);
        assert_eq!(got[1].progress.len(), 1);
        assert_eq!(got[1].progress[0].0, "a");
        assert_eq!(got[1].progress[0].1.step_label, "step two");
        assert_eq!(got[1].progress[0].1.percent, Some(50));
    }

    #[test]
    fn phase_maps_every_status_to_running_or_not() {
        let want = [(Status::Running, Phase::Running), (Status::WaitingInput, Phase::Running), (Status::Error, Phase::Failed), (Status::Idle, Phase::Paused), (Status::Finished, Phase::Finished)];
        for (st, ph) in want {
            assert_eq!(st.phase(), ph);
            assert_eq!(is_running(&mk("x", st)), ph == Phase::Running, "{st:?}");
        }
        assert!(!Phase::Cancelled.is_running());
    }

    #[test]
    fn running_and_finished_selectors_split_and_cover() {
        let mut seed = 12345u64;
        for _ in 0..100 {
            let v: Vec<SessionInfo> = (0..20)
                .map(|i| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    mk(&i.to_string(), ALL_ST[(seed >> 33) as usize % 5])
                })
                .collect();
            let r = running_sessions(&v);
            let f = finished_sessions(&v);
            assert!(r.iter().all(|s| s.status.phase().is_running()));
            assert!(f.iter().all(|s| !s.status.phase().is_running()));
            assert_eq!(r.len() + f.len(), v.len());
            assert!(r.iter().all(|a| !f.iter().any(|b| a.id == b.id)));
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bg-sess-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn digest_reparses_only_on_new_lines_or_age_class_change() {
        let d = tmp("digest");
        let p = d.join("t.jsonl");
        let l = |n: &str| format!("{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{{\"description\":\"{n}\"}}}}]}}}}\n");
        std::fs::write(&p, l("one")).unwrap();
        let mut sc = Scanner::new(roots_with(Some(&d)));
        let mt = mtime_ms(&p).unwrap();
        let (a, tail, _, _) = sc.digest(&p, mt + 1000, 0, false);
        assert_eq!((a.status, a.activity.as_str(), tail.len()), (Status::Running, "Bash: one", 1));
        let key = sc.tails[&p].memo.as_ref().unwrap().key;
        let _ = sc.digest(&p, mt + 1500, 0, false); // same lines, same age class: memo kept
        assert_eq!(sc.tails[&p].memo.as_ref().unwrap().key, key);
        let (a, _, _, _) = sc.digest(&p, mt + 300_000, 0, false); // 5 min idle: new class, re-analysed
        assert_eq!(a.status, Status::Idle);
        std::fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(l("two").as_bytes()).unwrap();
        let mt2 = mtime_ms(&p).unwrap();
        let (a, tail, _, _) = sc.digest(&p, mt2 + 500, 0, false);
        assert_eq!((a.status, a.activity.as_str(), tail.len()), (Status::Running, "Bash: two", 2));
        assert_eq!(age_class(2), (false, true, true));
        assert_eq!(age_class(7200), (true, false, false));
    }

    #[test]
    fn recent_activity_returns_last_n_oldest_first() {
        let l = |n: &str| format!("{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"tool_use\",\"name\":\"Bash\",\"input\":{{\"description\":\"{n}\"}}}}]}}}}");
        let lines: Vec<String> = ["a", "b", "c", "d"].iter().map(|n| l(n)).collect();
        assert_eq!(recent_activity(&lines, 3), vec!["Bash: b", "Bash: c", "Bash: d"]);
        assert!(recent_activity(&[], 3).is_empty());
    }

    fn si(id: &str, st: Status, last: i64) -> SessionInfo {
        SessionInfo { id: id.into(), name: id.into(), cwd: String::new(), env: Env::Desktop, status: st, archived: false, created_ms: 0, last_active_ms: last, activity: String::new(), progress: None, note: None, tail: Vec::new() }
    }

    fn assistant(blocks: &str, stop: &str) -> String {
        format!(r#"{{"type":"assistant","message":{{"content":[{blocks}],"stop_reason":{stop}}}}}"#)
    }

    #[test]
    fn json_parses_unicode_escapes_and_nesting() {
        let j = Json::parse(r#"{"a":[1,2.5,{"b":"x\u00e9\ud83d\ude00\n"}],"c":null,"d":true}"#).unwrap();
        assert_eq!(j.get("a").unwrap().arr()[2].gs("b"), Some("x\u{e9}\u{1F600}\n"));
        assert_eq!(j.get("d").and_then(Json::bool), Some(true));
        assert!(Json::parse("{bad").is_none());
        assert!(Json::parse(r#"{"a":1} x"#).is_none());
    }

    #[test]
    fn names_are_byte_equal_incl_hebrew_and_emoji() {
        let d = tmp("names");
        let sub = d.join("claude-code-sessions").join("acct").join("org");
        std::fs::create_dir_all(&sub).unwrap();
        let title = "  \u{5e9}\u{5dc}\u{5d5}\u{5dd}  \u{1F680} Mixed Title ";
        let body = format!(r#"{{"sessionId":"local_1","cliSessionId":"c1","cwd":"C:\\x","title":"{title}","createdAt":1,"lastActivityAt":2}}"#);
        std::fs::write(sub.join("local_1.json"), body).unwrap();
        let mut sc = Scanner::new(roots_with(Some(&d)));
        let (v, ok) = sc.scan(10_000);
        assert!(ok);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name.as_bytes(), title.as_bytes(), "no trim, no rename");
        assert_eq!(v[0].cwd, "C:\\x");
    }

    #[test]
    fn env_per_environment() {
        let p = |s: &str| Json::parse(s).unwrap();
        assert_eq!(desktop_env(&p(r#"{"sessionId":"local_a"}"#)), Env::Desktop);
        assert_eq!(desktop_env(&p(r#"{"sessionId":"local_a","cloudSessionId":"cse_1"}"#)), Env::Cloud);
        assert_eq!(desktop_env(&p(r#"{"sessionId":"local_a","sshHost":"box"}"#)), Env::Remote);
        assert_eq!(desktop_env(&p(r#"{"sessionId":"zzz"}"#)), Env::Unknown);
        assert_eq!(cli_env("cli"), Env::Cli);
        assert_eq!(cli_env("claude-vscode"), Env::Ide);
        assert_eq!(cli_env("claude-desktop"), Env::Desktop);
        assert_eq!(cli_env("something-new"), Env::Unknown);
    }

    #[test]
    fn status_per_state() {
        let tool = assistant(r#"{"type":"tool_use","name":"Bash","input":{"description":"Run tests"}}"#, "\"tool_use\"");
        assert_eq!(analyze(std::slice::from_ref(&tool), 5, false).status, Status::Running);
        assert_eq!(analyze(std::slice::from_ref(&tool), 600, false).status, Status::Idle, "stalled");
        let end = assistant(r#"{"type":"text","text":"All done"}"#, "\"end_turn\"");
        assert_eq!(analyze(std::slice::from_ref(&end), 30, false).status, Status::WaitingInput);
        assert_eq!(analyze(std::slice::from_ref(&end), 3 * 3600, false).status, Status::Idle);
        assert_eq!(analyze(std::slice::from_ref(&end), 30, true).status, Status::Finished);
        let ask = assistant(r#"{"type":"tool_use","name":"AskUserQuestion","input":{}}"#, "\"tool_use\"");
        assert_eq!(analyze(std::slice::from_ref(&ask), 10, false).status, Status::WaitingInput);
        let err = r#"{"type":"assistant","isApiErrorMessage":true,"message":{"content":[{"type":"text","text":"API Error"}]}}"#.to_string();
        assert_eq!(analyze(&[end.clone(), err], 5, false).status, Status::Error);
        let noise = r#"{"type":"api-request","x":1}"#.to_string();
        assert_eq!(analyze(&[tool, noise], 5, false).status, Status::Running, "noise records ignored");
        assert_eq!(analyze(&[], 5, false).status, Status::Idle);
    }

    #[test]
    fn progress_from_plan_and_checklist() {
        let plan = assistant(
            r#"{"type":"tool_use","name":"mcp__ccd_turn__update_plan","input":{"items":[{"content":"a","status":"completed"},{"content":"b","status":"in_progress"},{"content":"c","status":"pending"}]}}"#,
            "\"tool_use\"",
        );
        assert_eq!(analyze(&[plan], 1, false).progress, Some((1, 3)));
        let todo = assistant(r#"{"type":"tool_use","name":"TodoWrite","input":{"todos":[{"status":"completed"},{"status":"completed"}]}}"#, "\"tool_use\"");
        assert_eq!(analyze(&[todo], 1, false).progress, Some((2, 2)));
        let md = "# Run\n- [x] one\n- [ ] two\n- [X] three\nplain\n  - [ ] four\n";
        assert_eq!(checklist_progress(md), Some((2, 4)));
        assert_eq!(checklist_progress("nothing"), None);
        let txt = format!(r"Read F:\\backup\\ai\\ralp\\run-1\\{BOXFILE} now");
        assert_eq!(find_checklist_path(&txt), Some(format!("F:/backup/ai/ralp/run-1/{BOXFILE}")));
    }

    #[test]
    fn activity_truncates_long_and_multibyte_safely() {
        let long = "\u{5e9}\u{1F680}".repeat(200);
        let a = assistant(&format!(r#"{{"type":"text","text":"{long}"}}"#), "\"end_turn\"");
        let act = analyze(&[a], 1, false).activity;
        assert_eq!(act.chars().count(), 140);
        assert!(act.ends_with('\u{2026}'));
        assert_eq!(truncate_chars("a  b\nc", 10), "a b c");
        assert_eq!(truncate_chars("abc", 3), "abc");
        let t = assistant(r#"{"type":"tool_use","name":"Edit","input":{"file_path":"src/x.rs"}}"#, "\"tool_use\"");
        assert_eq!(analyze(&[t], 1, false).activity, "Edit: src/x.rs");
    }

    #[test]
    fn tail_reader_append_partial_truncate() {
        let d = tmp("tail");
        let p = d.join("t.jsonl");
        std::fs::write(&p, "one\ntwo\n").unwrap();
        let mut r = TailReader::new(p.clone());
        assert_eq!(r.read_new(), vec!["one", "two"]);
        assert!(r.read_new().is_empty(), "nothing appended");
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"thr").unwrap();
        assert!(r.read_new().is_empty(), "partial line is kept back");
        f.write_all(b"ee\nfour\n").unwrap();
        assert_eq!(r.read_new(), vec!["three", "four"], "only appended bytes, partial completed");
        std::fs::write(&p, "x\n").unwrap();
        assert_eq!(r.read_new(), vec!["x"], "truncated file is read again");
    }

    #[test]
    fn tail_reader_never_loads_a_giant_file() {
        let d = tmp("giant");
        let p = d.join("g.jsonl");
        let mut f = std::fs::File::create(&p).unwrap();
        let line = format!("{}\n", "y".repeat(99));
        for _ in 0..20_000 {
            f.write_all(line.as_bytes()).unwrap(); // ~2 MB
        }
        f.write_all(b"LAST\n").unwrap();
        drop(f);
        let mut r = TailReader::new(p);
        let got = r.read_new();
        assert_eq!(got.last().map(String::as_str), Some("LAST"));
        let bytes: usize = got.iter().map(|l| l.len() + 1).sum();
        assert!(bytes as u64 <= TAIL_CAP, "read {bytes} bytes");
        assert!(got.iter().all(|l| l == "LAST" || l.len() == 99), "no partial first line");
    }

    #[test]
    fn json_parse_accepts_utf8_bom() {
        let j = Json::parse("\u{feff}{\"sessionId\":\"s1\"}").expect("BOM json parses");
        assert_eq!(j.gs("sessionId"), Some("s1"));
    }

    #[test]
    fn app_restart_restores_live_sessions_and_moves_orphans_to_finished() {
        let d = tmp("restart");
        let sub = d.join("claude-code-sessions").join("a").join("o");
        let proj = d.join("projects").join("C--x");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(sub.join("local_1.json"), r#"{"sessionId":"local_1","cliSessionId":"cli-1","title":"Alpha","cwd":"C:\\x","lastActivityAt":1}"#).unwrap();
        let t = assistant(r#"{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}"#, "\"tool_use\"");
        std::fs::write(proj.join("cli-1.jsonl"), format!("{t}\n")).unwrap();
        let now = now_ms();
        let (before, _) = Scanner::new(roots_with(Some(&d))).scan(now);
        assert!(is_running(&before[0]), "running before the app is killed");
        // App killed mid-session, relaunched at once: a brand new Scanner (no memory) sees it still running.
        let (soon, _) = Scanner::new(roots_with(Some(&d))).scan(now + 5_000);
        assert!(is_running(&soon[0]), "restored as running");
        // Relaunched long after the session died: orphan lands in Finished, never shown as running.
        let (late, _) = Scanner::new(roots_with(Some(&d))).scan(now + 3_600_000);
        assert!(!is_running(&late[0]), "orphan is in Finished tab");
        assert_eq!(finished_sessions(&late).len(), 1);
    }

    #[test]
    fn e2e_three_sessions_progress_logs_finish_and_tab_placement() {
        use std::io::Write;
        let d = tmp("e2e");
        let sub = d.join("claude-code-sessions").join("a").join("o");
        let proj = d.join("projects").join("C--x");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        let meta = |i: usize, archived: bool| format!(r#"{{"sessionId":"local_{i}","cliSessionId":"cli-{i}","title":"Job {i}","cwd":"C:/x","isArchived":{archived},"lastActivityAt":1}}"#);
        let todo = |done: usize, cmd: &str| {
            let items: Vec<String> = (0..2).map(|k| format!(r#"{{"content":"s{k}","status":"{}"}}"#, if k < done { "completed" } else { "pending" })).collect();
            assistant(&format!(r#"{{"type":"tool_use","name":"TodoWrite","input":{{"todos":[{}]}}}},{{"type":"tool_use","name":"Bash","input":{{"command":"{cmd}"}}}}"#, items.join(",")), "\"tool_use\"")
        };
        for i in 1..=3 {
            std::fs::write(sub.join(format!("local_{i}.json")), meta(i, false)).unwrap();
            std::fs::write(proj.join(format!("cli-{i}.jsonl")), format!("{}\n", todo(0, "cargo build"))).unwrap();
        }
        let mut sc = Scanner::new(roots_with(Some(&d)));
        // 1. start: all three in the Running tab with 0/2 progress.
        let (v, ok) = sc.scan(now_ms());
        assert!(ok);
        assert_eq!(running_sessions(&v).len(), 3);
        assert!(v.iter().all(|s| s.progress == Some((0, 2)) && s.activity == "Bash: cargo build"));
        // 2. live: new log lines + progress arrive for session 2 only.
        let mut f = std::fs::OpenOptions::new().append(true).open(proj.join("cli-2.jsonl")).unwrap();
        writeln!(f, "{}", todo(1, "cargo test")).unwrap();
        drop(f);
        let (v, _) = sc.scan(now_ms());
        let s2 = v.iter().find(|s| s.name == "Job 2").unwrap();
        assert_eq!((s2.progress, s2.activity.as_str()), (Some((1, 2)), "Bash: cargo test"));
        assert_eq!(v.iter().find(|s| s.name == "Job 1").unwrap().progress, Some((0, 2)), "others untouched");
        // 3. finish: sessions 1 and 3 get archived/ended -> Finished tab; 2 keeps running.
        std::thread::sleep(std::time::Duration::from_millis(30));
        for i in [1, 3] {
            std::fs::write(sub.join(format!("local_{i}.json")), meta(i, true)).unwrap();
        }
        let (v, _) = sc.scan(now_ms());
        assert_eq!(running_sessions(&v).iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Job 2"]);
        assert_eq!(finished_sessions(&v).len(), 2, "finished ones sit in the Finished tab");
    }

    #[test]
    fn corrupt_files_are_skipped_and_good_sessions_survive() {
        let d = tmp("corrupt");
        let sub = d.join("claude-code-sessions").join("a").join("o");
        let proj = d.join("projects").join("C--x");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(sub.join("local_bad.json"), "{ not json ][").unwrap();
        std::fs::write(sub.join("local_1.json"), r#"{"sessionId":"local_1","cliSessionId":"cli-1","title":"Good","cwd":"C:/x","lastActivityAt":1}"#).unwrap();
        let t = assistant(r#"{"type":"tool_use","name":"Bash","input":{"command":"ok"}}"#, "\"tool_use\"");
        std::fs::write(proj.join("cli-1.jsonl"), format!("garbage line\n{{\"type\":\n{t}\n{{\"type\":\"assi")).unwrap();
        let (v, ok) = Scanner::new(roots_with(Some(&d))).scan(now_ms());
        assert!(ok);
        let good: Vec<_> = v.iter().filter(|s| s.name == "Good").collect();
        assert_eq!(good.len(), 1);
        assert_eq!(good[0].activity, "Bash: ok");
    }

    #[test]
    fn header_tiles_match_tab_counts() {
        let v: Vec<SessionInfo> = ALL_ST.iter().map(|&st| mk(&format!("{st:?}"), st)).collect();
        let (r, f) = tile_counts(&summarize(&v));
        assert_eq!((r as usize, f as usize), (running_sessions(&v).len(), finished_sessions(&v).len()));
    }

    #[test]
    fn scanner_reads_transcript_status_and_cloud_note() {
        let d = tmp("scan");
        let sub = d.join("claude-code-sessions").join("a").join("o");
        let proj = d.join("projects").join("C--x");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(sub.join("local_1.json"), r#"{"sessionId":"local_1","cliSessionId":"cli-1","title":"Alpha","cwd":"C:\\x","lastActivityAt":1}"#).unwrap();
        std::fs::write(sub.join("local_2.json"), r#"{"sessionId":"local_2","cliSessionId":"","title":"Cloudy","cloudSessionId":"cse_1","lastActivityAt":1}"#).unwrap();
        let t = assistant(r#"{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}"#, "\"tool_use\"");
        std::fs::write(proj.join("cli-1.jsonl"), format!("{t}\n")).unwrap();
        let mut sc = Scanner::new(roots_with(Some(&d)));
        let (v, ok) = sc.scan(now_ms());
        assert!(ok);
        let a = v.iter().find(|s| s.name == "Alpha").unwrap();
        assert_eq!((a.status, a.activity.as_str(), a.env), (Status::Running, "Bash: cargo test", Env::Desktop));
        let c = v.iter().find(|s| s.name == "Cloudy").unwrap();
        assert_eq!(c.env, Env::Cloud);
        assert!(c.note.as_deref().unwrap().contains("not live from this PC"));
        assert_eq!(cloud_note(Env::Desktop), None);
        let (_, ok2) = Scanner::new(roots_with(Some(&d.join("missing")))).scan(0);
        assert!(!ok2, "no sources readable");
    }

    #[test]
    fn diff_events_on_status_change() {
        let old = vec![si("a", Status::Running, 1), si("b", Status::Running, 1), si("c", Status::Idle, 1)];
        let new = vec![si("a", Status::Finished, 2), si("b", Status::WaitingInput, 2), si("c", Status::Idle, 2), si("d", Status::Finished, 2)];
        assert_eq!(diff(&old, &new), vec![Event::Finished("a".into(), "a".into()), Event::NeedsInput("b".into(), "b".into())]);
        assert!(diff(&new, &new).is_empty());
    }

    #[test]
    fn staleness_flag() {
        let now = Instant::now();
        let s = Snapshot { sessions: vec![], taken: now, ok: true, live: 0, progress: vec![] };
        assert!(!is_stale(&s, now + Duration::from_secs(4)));
        assert!(is_stale(&s, now + Duration::from_secs(6)));
        assert!(is_stale(&Snapshot { ok: false, ..s }, now), "failed refresh is always stale");
    }

    #[test]
    fn ordering_filter_and_summary() {
        let mut v = vec![si("done", Status::Finished, 9), si("idle", Status::Idle, 8), si("run-old", Status::Running, 1), si("run-new", Status::Running, 5), si("wait", Status::WaitingInput, 7)];
        let pinned: HashSet<String> = ["idle".to_string()].into();
        sort_sessions(&mut v, &pinned);
        let ids: Vec<&str> = v.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["idle", "run-new", "run-old", "wait", "done"]);
        let mut chips = HashSet::new();
        assert!(matches(&v[1], "", &chips));
        assert!(matches(&v[1], "RUN-N", &chips), "case-insensitive search");
        assert!(!matches(&v[1], "zzz", &chips));
        chips.insert(Status::Finished);
        assert!(!matches(&v[1], "", &chips));
        assert!(matches(&v[4], "", &chips));
        v[1].progress = Some((1, 4));
        v[2].progress = Some((3, 6));
        let s = summarize(&v);
        assert_eq!((s.total, s.running, s.waiting, s.done, s.idle), (5, 2, 1, 1, 1));
        assert_eq!(s.progress, (4, 10));
    }

    #[test]
    fn age_and_elapsed_formats() {
        assert_eq!(age_text(0), "0s ago");
        assert_eq!(age_text(59), "59s ago");
        assert_eq!(age_text(60), "1m ago");
        assert_eq!(age_text(7200), "2h ago");
        assert_eq!(age_text(200_000), "2d ago");
        assert_eq!(age_text(-5), "0s ago");
        assert_eq!(elapsed_text(12), "12s");
        assert_eq!(elapsed_text(303), "5m 03s");
        assert_eq!(elapsed_text(3900), "1h 05m");
    }

    /// Manual live check (cargo test -- --ignored live_dump --nocapture): what the page would show for real sessions.
    #[test]
    #[ignore]
    fn live_dump() {
        let mut sc = Scanner::new(roots());
        let (v, ok) = sc.scan(now_ms());
        println!("LIVE ok={ok} n={}", v.len());
        for s in v.iter().filter(|s| !s.archived) {
            println!("LIVE|{}|{}|{:?}", s.id, s.name, s.status);
        }
    }

    #[test]
    fn roots_override_covers_all_three() {
        let r = roots_with(Some(Path::new("X:/fix")));
        assert!(r.desktop.ends_with("claude-code-sessions") && r.projects.ends_with("projects") && r.cli.ends_with("sessions"));
        assert!(r.desktop.starts_with("X:/fix") && r.projects.starts_with("X:/fix") && r.cli.starts_with("X:/fix"));
    }
}
