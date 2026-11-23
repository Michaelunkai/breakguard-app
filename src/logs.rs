//! Per-session live log model: typed lines in bounded ring buffers, an incremental transcript
//! tailer, the record parser and name / state resolution. Pure logic, no UI.
use crate::sessions::Json;
use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------ model

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    Assistant,
    ToolCall,
    ToolResult,
    User,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    /// Unix milliseconds from the record, 0 when unknown.
    pub ts_ms: i64,
    pub kind: LineKind,
    /// Tool name for ToolCall lines (Bash, Read, Edit ...), empty otherwise.
    pub tool: String,
    pub text: String,
    /// One plain-English sentence saying what a tool call / result means; empty for chat lines.
    pub plain: String,
}

impl LogLine {
#[cfg(test)]
    pub fn new(kind: LineKind, text: &str) -> LogLine {
        LogLine { ts_ms: 0, kind, tool: String::new(), text: text.to_string(), plain: String::new() }
    }
    /// Approximate heap bytes this line holds.
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<LogLine>() + self.text.len() + self.tool.len()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogState {
    Running,
    Idle,
    Finished,
}

pub const LINE_CAP: usize = 4000; // characters kept per line
pub const SESSION_MAX_LINES: usize = 2000;
pub const SESSION_MAX_BYTES: usize = 1 << 20;
pub const GLOBAL_MAX_BYTES: usize = 48 << 20;

/// One session's lane data: a bounded ring buffer of typed lines plus running counters.
#[derive(Clone, Debug)]
pub struct SessionLog {
    pub id: String,
    pub name: String,
    pub state: LogState,
    pub started_ms: i64,
    lines: VecDeque<LogLine>,
    max_lines: usize,
    max_bytes: usize,
    bytes: usize,
    /// Lines ever pushed (not just those still held).
    pub total_lines: u64,
    pub tool_calls: u64,
    pub errors: u64,
    /// Lines dropped from the front to stay inside the caps.
    pub dropped: u64,
    /// Working folder of the session (empty when unknown).
    pub cwd: String,
}

impl SessionLog {
    pub fn new(id: &str, name: &str) -> SessionLog {
        SessionLog::with_caps(id, name, SESSION_MAX_LINES, SESSION_MAX_BYTES)
    }

    pub fn with_caps(id: &str, name: &str, max_lines: usize, max_bytes: usize) -> SessionLog {
        SessionLog {
            id: id.to_string(),
            name: name.to_string(),
            state: LogState::Idle,
            started_ms: 0,
            lines: VecDeque::new(),
            max_lines: max_lines.max(1),
            max_bytes: max_bytes.max(1024),
            bytes: 0,
            total_lines: 0,
            tool_calls: 0,
            errors: 0,
            dropped: 0,
            cwd: String::new(),
        }
    }

    /// Append a line (oldest first order is kept); trims the oldest when over a cap.
    pub fn push(&mut self, mut l: LogLine) {
        if l.text.chars().count() > LINE_CAP {
            l.text = truncate(&l.text, LINE_CAP);
        }
        match l.kind {
            LineKind::ToolCall => self.tool_calls += 1,
            LineKind::Error => self.errors += 1,
            _ => {}
        }
        self.total_lines += 1;
        self.bytes += l.bytes();
        self.lines.push_back(l);
        while self.lines.len() > self.max_lines || (self.bytes > self.max_bytes && self.lines.len() > 1) {
            self.drop_oldest();
        }
    }

    fn drop_oldest(&mut self) -> bool {
        match self.lines.pop_front() {
            Some(o) => {
                self.bytes = self.bytes.saturating_sub(o.bytes());
                self.dropped += 1;
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }
#[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
#[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LogLine> {
        self.lines.iter()
    }
#[cfg(test)]
    pub fn last(&self) -> Option<&LogLine> {
        self.lines.back()
    }
#[cfg(test)]
    pub fn clear(&mut self) {
        self.lines.clear();
        self.bytes = 0;
    }
}

/// Every session log under one global byte cap: the biggest log loses its oldest lines first.
#[derive(Debug)]
pub struct LogStore {
    pub logs: Vec<SessionLog>,
    pub global_max_bytes: usize,
}

impl Default for LogStore {
    fn default() -> Self {
        LogStore { logs: Vec::new(), global_max_bytes: GLOBAL_MAX_BYTES }
    }
}

impl LogStore {
#[cfg(test)]
    pub fn with_global_cap(bytes: usize) -> LogStore {
        LogStore { logs: Vec::new(), global_max_bytes: bytes }
    }
    pub fn total_bytes(&self) -> usize {
        self.logs.iter().map(|l| l.bytes).sum()
    }
    pub fn get_or_add(&mut self, id: &str, name: &str) -> &mut SessionLog {
        if let Some(i) = self.logs.iter().position(|l| l.id == id) {
            return &mut self.logs[i];
        }
        self.logs.push(SessionLog::new(id, name));
        self.logs.last_mut().unwrap()
    }
    pub fn push(&mut self, id: &str, l: LogLine) {
        if let Some(log) = self.logs.iter_mut().find(|x| x.id == id) {
            log.push(l);
        }
        self.enforce();
    }
    /// Trim the largest logs until the global cap holds.
    pub fn enforce(&mut self) {
        while self.total_bytes() > self.global_max_bytes {
            let Some(big) = self.logs.iter_mut().filter(|l| l.lines.len() > 1).max_by_key(|l| l.bytes) else { break };
            big.drop_oldest();
        }
    }
    pub fn remove(&mut self, id: &str) -> bool {
        let n = self.logs.len();
        self.logs.retain(|l| l.id != id);
        self.logs.len() != n
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t.push('\u{2026}');
    t
}

// ------------------------------------------------------------------ record parser

/// ISO-8601 UTC `2026-11-19T14:19:09.777Z` to unix milliseconds.
pub fn parse_ts(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let ms = if b.len() > 20 && b[19] == b'.' {
        let frac: String = s[20..].chars().take_while(|c| c.is_ascii_digit()).take(3).collect();
        format!("{frac:0<3}").parse::<i64>().ok()?
    } else {
        0
    };
    // days from civil (Howard Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(((days * 24 + h) * 60 + mi) * 60_000 + se * 1000 + ms)
}

fn tool_detail(name: &str, input: Option<&Json>) -> String {
    let get = |k: &str| input.and_then(|i| i.gs(k)).map(str::to_string);
    let d = get("description")
        .or_else(|| get("command"))
        .or_else(|| get("file_path"))
        .or_else(|| get("path"))
        .or_else(|| get("pattern"))
        .or_else(|| get("query"))
        .or_else(|| get("url"))
        .unwrap_or_default();
    if d.is_empty() {
        name.to_string()
    } else {
        format!("{name}: {d}")
    }
}

/// Last path part of `p` (either slash), for sentences like "Reads the file main.rs".
fn base_name(p: &str) -> &str {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or(p)
}

/// Plain-English sentence for a tool call. Pure: same name + input, same sentence.
pub fn plain_tool(name: &str, input: Option<&Json>) -> String {
    let get = |k: &str| input.and_then(|i| i.gs(k)).map(str::trim).filter(|s| !s.is_empty());
    let file = || get("file_path").or_else(|| get("path")).or_else(|| get("notebook_path")).map(base_name);
    let on = |verb: &str, what: Option<&str>| match what {
        Some(w) => format!("{verb} {w}"),
        None => verb.to_string(),
    };
    match name {
        "Bash" | "PowerShell" | "BashOutput" | "Monitor" => match get("description") {
            Some(d) => format!("Runs a command: {}", truncate(d, 90)),
            None => "Runs a command in the terminal".to_string(),
        },
        "Read" => on("Reads the file", file()),
        "Edit" | "MultiEdit" | "NotebookEdit" => on("Edits the file", file()),
        "Write" => on("Writes the file", file()),
        "Grep" => match get("pattern") {
            Some(p) => format!("Searches the code for \"{}\"", truncate(p, 60)),
            None => "Searches the code".to_string(),
        },
        "Glob" => match get("pattern") {
            Some(p) => format!("Looks for files matching {}", truncate(p, 60)),
            None => "Looks for files".to_string(),
        },
        "WebFetch" => match get("url") {
            Some(u) => format!("Opens the web page {}", truncate(u.trim_start_matches("https://").trim_start_matches("http://"), 70)),
            None => "Opens a web page".to_string(),
        },
        "WebSearch" => match get("query") {
            Some(q) => format!("Searches the web for \"{}\"", truncate(q, 70)),
            None => "Searches the web".to_string(),
        },
        "Agent" | "Task" => match get("description") {
            Some(d) => format!("Starts a helper: {}", truncate(d, 80)),
            None => "Starts a helper agent".to_string(),
        },
        "TodoWrite" | "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet" => "Updates its to-do list".to_string(),
        "ToolSearch" => "Looks up more tools to use".to_string(),
        "Skill" => on("Starts the skill", get("skill")),
        "AskUserQuestion" => "Asks you a question".to_string(),
        n if n.starts_with("mcp__") => "Uses a helper tool".to_string(),
        _ => "Uses a tool".to_string(),
    }
}

/// Shortens a path-like token to its file name, keeping surrounding punctuation.
fn shorten_path(tok: &str) -> String {
    let core = tok.trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | ',' | ':' | ';'));
    if !(core.contains('\\') || core.contains('/')) {
        return tok.to_string();
    }
    let name = base_name(core);
    if name.is_empty() {
        tok.to_string()
    } else {
        tok.replacen(core, name, 1)
    }
}

/// First line of a failure that a non-programmer can read: no raw JSON, no stack lines,
/// paths cut to file names, at most 90 characters. Empty when nothing readable is left.
fn failure_reason(text: &str) -> String {
    let readable = |l: &&str| {
        !l.is_empty()
            && !l.starts_with(['{', '[', '}', ']', '"'])
            && !l.starts_with("at ")
            && !l.starts_with("File \"")
            && !l.starts_with("stack backtrace")
    };
    let line = text.lines().map(str::trim).find(readable).unwrap_or("");
    let shortened: Vec<String> = line.split_whitespace().map(shorten_path).collect();
    truncate(&shortened.join(" "), 90)
}

/// Plain-English sentence for a tool result: how much came back, or what went wrong.
pub fn plain_result(text: &str, is_err: bool) -> String {
    let t = text.trim();
    if is_err {
        let reason = failure_reason(t);
        return if reason.is_empty() { "It failed".to_string() } else { format!("It failed: {reason}") };
    }
    match t.lines().count() {
        0 => "Done, nothing was printed".to_string(),
        1 => "Done, got 1 line back".to_string(),
        n => format!("Done, got {n} lines back"),
    }
}

/// Text of a tool_result `content` (string, or array of text blocks).
fn result_text(c: Option<&Json>) -> String {
    match c {
        Some(Json::Str(s)) => s.clone(),
        Some(v) => v.arr().iter().filter_map(|b| b.gs("text")).collect::<Vec<_>>().join("\n"),
        None => String::new(),
    }
}

/// Typed lines of one transcript record. Unknown record types and malformed JSON give no lines.
pub fn parse_record(line: &str) -> Vec<LogLine> {
    let Some(j) = Json::parse(line.trim_start_matches('\u{feff}')) else { return Vec::new() };
    let ts = j.gs("timestamp").and_then(parse_ts).unwrap_or(0);
    let mk = |kind, tool: &str, text: String| LogLine { ts_ms: ts, kind, tool: tool.to_string(), text, plain: String::new() };
    let mut out = Vec::new();
    match j.gs("type") {
        Some("assistant") => {
            let Some(msg) = j.get("message") else { return out };
            if msg.gs("error").is_some() || j.get("isApiErrorMessage").and_then(Json::bool) == Some(true) {
                let t: String = msg.get("content").map(|c| c.arr().iter().filter_map(|b| b.gs("text")).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                out.push(mk(LineKind::Error, "", if t.is_empty() { "API error".into() } else { t }));
                return out;
            }
            for b in msg.get("content").map(|c| c.arr()).unwrap_or(&[]) {
                match b.gs("type") {
                    Some("text") => {
                        let t = b.gs("text").unwrap_or("").trim();
                        if !t.is_empty() {
                            out.push(mk(LineKind::Assistant, "", t.to_string()));
                        }
                    }
                    Some("tool_use") => {
                        let name = b.gs("name").unwrap_or("tool");
                        let mut l = mk(LineKind::ToolCall, name, tool_detail(name, b.get("input")));
                        l.plain = plain_tool(name, b.get("input"));
                        out.push(l);
                    }
                    _ => {} // thinking blocks are not shown
                }
            }
        }
        Some("user") => {
            let Some(msg) = j.get("message") else { return out };
            match msg.get("content") {
                Some(Json::Str(s)) => {
                    if !s.trim().is_empty() {
                        out.push(mk(LineKind::User, "", s.trim().to_string()));
                    }
                }
                Some(c) => {
                    for b in c.arr() {
                        match b.gs("type") {
                            Some("tool_result") => {
                                let t = result_text(b.get("content"));
                                let err = b.get("is_error").and_then(Json::bool) == Some(true);
                                let mut l = mk(if err { LineKind::Error } else { LineKind::ToolResult }, "", t.trim().to_string());
                                l.plain = plain_result(&l.text, err);
                                out.push(l);
                            }
                            Some("text") => {
                                let t = b.gs("text").unwrap_or("").trim();
                                if !t.is_empty() {
                                    out.push(mk(LineKind::User, "", t.to_string()));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                None => {}
            }
        }
        Some("system") if j.gs("level") == Some("error") => {
            out.push(mk(LineKind::Error, "", j.gs("content").unwrap_or("system error").to_string()));
        }
        _ => {}
    }
    out
}

/// Custom title written into the transcript (`custom-title` / `agent-name` records), if the record is one.
pub fn title_of_record(line: &str) -> Option<String> {
    let j = Json::parse(line.trim_start_matches('\u{feff}'))?;
    match j.gs("type")? {
        "custom-title" => j.gs("customTitle").map(str::to_string),
        "agent-name" => j.gs("agentName").map(str::to_string),
        _ => None,
    }
    .filter(|t| !t.trim().is_empty())
}

// ------------------------------------------------------------------ names and state

/// What Claude shows: custom title, else the project folder, else the short session id.
#[cfg(test)]
pub fn resolve_name(custom: Option<&str>, cwd: &str, session_id: &str) -> String {
    if let Some(t) = custom.map(str::trim).filter(|t| !t.is_empty()) {
        return t.to_string();
    }
    if let Some(f) = Path::new(cwd).file_name().and_then(|n| n.to_str()).filter(|f| !f.is_empty()) {
        return f.to_string();
    }
    session_id.chars().take(8).collect()
}

/// Running while the process lives and the transcript was written recently; Idle while the process
/// lives but is quiet; Finished once the process is gone.
pub fn detect_state(process_alive: bool, last_write_age_secs: i64) -> LogState {
    if !process_alive {
        LogState::Finished
    } else if last_write_age_secs <= RUNNING_WINDOW_SECS {
        LogState::Running
    } else {
        LogState::Idle
    }
}

pub const RUNNING_WINDOW_SECS: i64 = 30;

// ------------------------------------------------------------------ incremental tailer

pub const MAX_LINE_BYTES: usize = 4 << 20; // longer lines are skipped whole
const CHUNK: u64 = 1 << 20;
const HEAD: usize = 64;

/// Reads only the new bytes of a growing jsonl file, by offset. Partial last lines wait for their
/// newline; truncation and rotation (file replaced) restart from the top; huge lines are skipped.
#[derive(Debug)]
pub struct Tailer {
    path: PathBuf,
    pos: u64,
    carry: Vec<u8>,
    head: Vec<u8>,
    skipping: bool,
    /// Lines skipped for size since the start.
    pub skipped_huge: u64,
}

impl Tailer {
    pub fn new(path: PathBuf) -> Tailer {
        Tailer { path, pos: 0, carry: Vec::new(), head: Vec::new(), skipping: false, skipped_huge: 0 }
    }

    /// Start `back` bytes before the end (history for a new lane); the first, probably cut, line is dropped.
    pub fn seek_tail(&mut self, back: u64) {
        let Ok(m) = std::fs::metadata(&self.path) else { return };
        if m.len() > back {
            self.pos = m.len() - back;
            self.skipping = true;
            self.head = Vec::new(); // not at the file start: skip the rotation check once
        }
    }

    fn reset(&mut self) {
        self.pos = 0;
        self.carry.clear();
        self.skipping = false;
    }

    /// New complete lines since the last call (at most ~1 MiB of file per call).
    pub fn poll(&mut self) -> Vec<String> {
        let Ok(mut f) = std::fs::File::open(&self.path) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let head = {
            let mut b = vec![0u8; HEAD.min(len as usize)];
            let _ = f.read_exact(&mut b);
            b
        };
        let n = self.head.len().min(head.len());
        let rotated = self.pos > 0 && (len < self.pos || head[..n] != self.head[..n]);
        if rotated {
            self.reset();
        }
        self.head = head;
        if len <= self.pos || f.seek(SeekFrom::Start(self.pos)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        let _ = f.take((len - self.pos).min(CHUNK)).read_to_end(&mut buf);
        self.pos += buf.len() as u64;
        let mut lines = Vec::new();
        for &c in &buf {
            if c == b'\n' {
                if self.skipping {
                    self.skipping = false;
                } else {
                    let t = String::from_utf8_lossy(&self.carry).trim_end_matches('\r').to_string();
                    if !t.is_empty() {
                        lines.push(t);
                    }
                }
                self.carry.clear();
            } else if !self.skipping {
                self.carry.push(c);
                if self.carry.len() > MAX_LINE_BYTES {
                    self.skipping = true;
                    self.skipped_huge += 1;
                    self.carry.clear();
                }
            }
        }
        lines
    }
}

// ------------------------------------------------------------------ ralp progress footer

/// Checklist ticks and ledger tail of a ralp run a session works on.
#[derive(Clone, Debug, PartialEq)]
pub struct RalpFooter {
    pub run_dir: PathBuf,
    pub done: u32,
    pub total: u32,
    pub ledger_tail: Vec<String>,
}

/// Read `CHECKLIST.md` (ticks) and `LEDGER.md` (last `n` lines) next to the given checklist path.
pub fn read_ralp(checklist: &Path, n: usize) -> Option<RalpFooter> {
    let text = std::fs::read_to_string(checklist).ok()?;
    let (done, total) = crate::sessions::checklist_progress(&text)?;
    let run_dir = checklist.parent()?.to_path_buf();
    let ledger_tail = std::fs::read_to_string(run_dir.join("LEDGER.md"))
        .map(|t| {
            let v: Vec<String> = t.lines().filter(|l| !l.trim().is_empty()).map(|l| truncate(l.trim(), 200)).collect();
            v[v.len().saturating_sub(n)..].to_vec()
        })
        .unwrap_or_default();
    Some(RalpFooter { run_dir, done, total, ledger_tail })
}

// ------------------------------------------------------------------ live hub

pub type AliveBox = Box<dyn Fn(u32, u64) -> bool + Send>;

/// Follows every running agent: discovers sessions, tails their transcripts, fills the store.
pub struct LogHub {
    pub store: LogStore,
    roots: crate::sessions::Roots,
    alive: AliveBox,
    tailers: std::collections::HashMap<String, Tailer>,
    index: (std::time::Instant, std::collections::HashMap<String, PathBuf>),
    last_write_ms: std::collections::HashMap<String, i64>,
    /// When each ended session was first seen gone (unix ms); the Finished tab ages by it.
    pub gone_since: std::collections::HashMap<String, i64>,
    checklists: std::collections::HashMap<String, PathBuf>,
    pub ralp: std::collections::HashMap<String, RalpFooter>,
    /// A finished session's lane stays this long after its process exits.
    pub finished_keep_ms: i64,
    /// How much history a new lane reads from the end of a transcript.
    pub history_bytes: u64,
}

impl LogHub {
    pub fn new(roots: crate::sessions::Roots, alive: AliveBox) -> LogHub {
        LogHub {
            store: LogStore::default(),
            roots,
            alive,
            tailers: Default::default(),
            index: (std::time::Instant::now() - std::time::Duration::from_secs(3600), Default::default()),
            last_write_ms: Default::default(),
            gone_since: Default::default(),
            checklists: Default::default(),
            ralp: Default::default(),
            // an ended session leaves the Live tab at once and stays on the Finished tab for a few hours
            finished_keep_ms: crate::ui::logic::tabs::FINISHED_KEEP_MS,
            history_bytes: 256 * 1024,
        }
    }

    fn transcript(&mut self, sid: &str, force: bool) -> Option<PathBuf> {
        if force || self.index.0.elapsed() > std::time::Duration::from_secs(5) {
            let mut m = std::collections::HashMap::new();
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
            self.index = (std::time::Instant::now(), m);
        }
        self.index.1.get(sid).cloned()
    }

    /// One pass. `scan` also re-discovers sessions (about once a second); every pass reads new bytes.
    /// Returns how many new lines arrived.
    pub fn tick(&mut self, scan: bool, now_ms: i64) -> usize {
        if scan {
            let agents = crate::agents::discover(&self.roots, &*self.alive);
            let mut seen: Vec<String> = Vec::new();
            for a in &agents {
                seen.push(a.session_id.clone());
                self.gone_since.remove(&a.session_id);
                // the name Claude shows right now (sidebar renames included), every scan
                if let Some(l) = self.store.logs.iter_mut().find(|l| l.id == a.session_id) {
                    if l.name != a.name {
                        l.name = a.name.clone();
                    }
                    if l.state == LogState::Finished {
                        l.state = LogState::Idle;
                    }
                }
                if !self.tailers.contains_key(&a.session_id) {
                    let log = self.store.get_or_add(&a.session_id, &a.name);
                    log.started_ms = if a.started_ms > 0 { a.started_ms } else { now_ms };
                    log.name = a.name.clone();
                    log.cwd = a.cwd.clone();
                    if let Some(tp) = self.transcript(&a.session_id, true) {
                        let mut t = Tailer::new(tp);
                        t.seek_tail(self.history_bytes);
                        self.tailers.insert(a.session_id.clone(), t);
                    }
                }
            }
            let ids: Vec<String> = self.store.logs.iter().map(|l| l.id.clone()).collect();
            for id in ids {
                if seen.contains(&id) {
                    continue;
                }
                let since = *self.gone_since.entry(id.clone()).or_insert(now_ms);
                if let Some(l) = self.store.logs.iter_mut().find(|l| l.id == id) {
                    l.state = LogState::Finished;
                }
                if now_ms - since >= self.finished_keep_ms {
                    self.store.remove(&id);
                    self.tailers.remove(&id);
                    self.last_write_ms.remove(&id);
                    self.gone_since.remove(&id);
                    self.checklists.remove(&id);
                    self.ralp.remove(&id);
                }
            }
            // sessions whose transcript appeared late
            let missing: Vec<String> = seen.iter().filter(|s| !self.tailers.contains_key(*s)).cloned().collect();
            for sid in missing {
                if let Some(tp) = self.transcript(&sid, false) {
                    let mut t = Tailer::new(tp);
                    t.seek_tail(self.history_bytes);
                    self.tailers.insert(sid, t);
                }
            }
        }
        let mut new_lines = 0;
        let ids: Vec<String> = self.tailers.keys().cloned().collect();
        for id in ids {
            let lines = self.tailers.get_mut(&id).map(|t| t.poll()).unwrap_or_default();
            if lines.is_empty() {
                continue;
            }
            self.last_write_ms.insert(id.clone(), now_ms);
            for raw in &lines {
                // title records are not output; the lane name comes from discovery (what Claude shows),
                // so an old title record in the history window can never overwrite the current one
                if title_of_record(raw).is_some() {
                    continue;
                }
                if raw.contains(crate::sessions::BOXFILE) {
                    if let Some(p) = crate::sessions::find_checklist_path(raw) {
                        self.checklists.insert(id.clone(), PathBuf::from(p));
                    }
                }
                for l in parse_record(raw) {
                    new_lines += 1;
                    self.store.push(&id, l);
                }
            }
        }
        for l in self.store.logs.iter_mut() {
            if self.gone_since.contains_key(&l.id) {
                continue;
            }
            let age = self.last_write_ms.get(&l.id).map(|w| (now_ms - w) / 1000).unwrap_or(i64::MAX / 2);
            l.state = detect_state(true, age);
        }
        if scan {
            for (id, p) in self.checklists.clone() {
                if let Some(f) = read_ralp(&p, 3) {
                    self.ralp.insert(id, f);
                }
            }
        }
        new_lines
    }
}

/// Background loop: polls every 250 ms while `wanted` is set (the Log page is open), else idles.
pub fn spawn_hub(ctx: eframe::egui::Context, wanted: std::sync::Arc<std::sync::atomic::AtomicBool>) -> std::sync::Arc<std::sync::Mutex<LogHub>> {
    spawn_hub_with(ctx, wanted, LogHub::new(crate::sessions::roots(), Box::new(crate::agents::process_alive)))
}

/// Same loop for a given hub (tests feed a fixture hub through the real background thread).
pub fn spawn_hub_with(ctx: eframe::egui::Context, wanted: std::sync::Arc<std::sync::atomic::AtomicBool>, hub: LogHub) -> std::sync::Arc<std::sync::Mutex<LogHub>> {
    use std::sync::atomic::Ordering;
    let hub = std::sync::Arc::new(std::sync::Mutex::new(hub));
    let h2 = hub.clone();
    std::thread::spawn(move || {
        let mut last_scan = std::time::Instant::now() - std::time::Duration::from_secs(60);
        loop {
            if !wanted.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
            let scan = last_scan.elapsed() >= std::time::Duration::from_secs(1);
            if scan {
                last_scan = std::time::Instant::now();
            }
            let now = crate::sessions::now_ms();
            let n = h2.lock().map(|mut h| h.tick(scan, now)).unwrap_or(0);
            if n > 0 || scan {
                ctx.request_repaint();
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
    hub
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bg-logs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn ln(k: LineKind, t: &str) -> LogLine {
        LogLine::new(k, t)
    }

    // ---- box 13: model
    #[test]
    fn model_push_cap_ordering_and_counters() {
        let mut l = SessionLog::with_caps("s", "n", 5, 1 << 20);
        for i in 0..8 {
            l.push(ln(if i % 4 == 0 { LineKind::ToolCall } else if i == 5 { LineKind::Error } else { LineKind::Assistant }, &format!("line {i}")));
        }
        assert_eq!(l.len(), 5, "ring buffer capped");
        let got: Vec<&str> = l.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(got, ["line 3", "line 4", "line 5", "line 6", "line 7"], "oldest dropped, order kept");
        assert_eq!((l.total_lines, l.dropped, l.tool_calls, l.errors), (8, 3, 2, 1));
        assert_eq!(l.last().unwrap().text, "line 7");
        assert_eq!((l.id.as_str(), l.name.as_str(), l.state), ("s", "n", LogState::Idle));
        let mut b = SessionLog::with_caps("b", "b", 1_000_000, 4096);
        for _ in 0..200 {
            b.push(ln(LineKind::User, &"x".repeat(100)));
        }
        assert!(b.bytes() <= 4096 + 400, "byte cap holds: {}", b.bytes());
        let mut c = SessionLog::new("c", "c");
        c.push(ln(LineKind::Assistant, &"y".repeat(10_000)));
        assert_eq!(c.last().unwrap().text.chars().count(), LINE_CAP, "huge line truncated with ellipsis");
        c.clear();
        assert!(c.is_empty() && c.bytes() == 0);
    }

    // ---- box 20: memory bound
    #[test]
    fn million_lines_stay_under_the_caps() {
        let mut st = LogStore::with_global_cap(8 << 20);
        for s in 0..4 {
            st.get_or_add(&format!("s{s}"), "x");
        }
        for i in 0..1_000_000u32 {
            st.push(&format!("s{}", i % 4), ln(LineKind::Assistant, "a typical log line with some text in it"));
        }
        for l in &st.logs {
            assert!(l.len() <= SESSION_MAX_LINES, "per-session line cap");
            assert!(l.bytes() <= SESSION_MAX_BYTES + 400);
            assert_eq!(l.total_lines, 250_000);
        }
        assert!(st.total_bytes() <= 8 << 20, "global cap: {}", st.total_bytes());
        // many big sessions: global cap trims the largest first
        let mut g = LogStore::with_global_cap(2 << 20);
        for s in 0..10 {
            g.get_or_add(&format!("g{s}"), "x");
        }
        for i in 0..200_000u32 {
            g.push(&format!("g{}", i % 10), ln(LineKind::ToolResult, &"z".repeat(200)));
        }
        assert!(g.total_bytes() <= 2 << 20, "{}", g.total_bytes());
        assert!(g.logs.iter().all(|l| l.len() > 100), "no session starved: {:?}", g.logs.iter().map(|l| l.len()).collect::<Vec<_>>());
    }

    // ---- box 18: parser
    #[test]
    fn parser_kinds_from_real_shaped_records() {
        let asst = r#"{"parentUuid":"a","type":"assistant","message":{"model":"claude-sonnet-5-5","role":"assistant","content":[{"type":"thinking","thinking":"","signature":"x"},{"type":"text","text":"Now the patch script."},{"type":"tool_use","id":"toolu_1","name":"Write","input":{"file_path":"C:/x/patch.py","content":"..."}}]},"timestamp":"2026-11-19T14:19:26.315Z"}"#;
        let v = parse_record(asst);
        assert_eq!(v.len(), 2, "thinking not shown: {v:?}");
        assert_eq!((v[0].kind, v[0].text.as_str()), (LineKind::Assistant, "Now the patch script."));
        assert_eq!((v[1].kind, v[1].tool.as_str(), v[1].text.as_str()), (LineKind::ToolCall, "Write", "Write: C:/x/patch.py"));
        assert_eq!(v[0].ts_ms, 1_795_097_966_315, "timestamp parsed");
        let bash = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test","description":"Run tests"}}]}}"#;
        assert_eq!(parse_record(bash)[0].text, "Bash: Run tests");
        let res = r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"patched\r\nok","is_error":false}]},"timestamp":"2026-11-19T14:19:27.000Z"}"#;
        let v = parse_record(res);
        assert_eq!((v[0].kind, v[0].text.as_str()), (LineKind::ToolResult, "patched\r\nok"));
        let err = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":[{"type":"text","text":"command failed"}],"is_error":true}]}}"#;
        let v = parse_record(err);
        assert_eq!((v[0].kind, v[0].text.as_str()), (LineKind::Error, "command failed"));
        let prompt = r#"{"type":"user","message":{"role":"user","content":"Continue; do not restart."}}"#;
        assert_eq!(parse_record(prompt), vec![LogLine { ts_ms: 0, kind: LineKind::User, tool: String::new(), text: "Continue; do not restart.".into(), plain: String::new() }]);
        let blocks = r#"{"type":"user","message":{"content":[{"type":"text","text":"typed prompt"}]}}"#;
        assert_eq!(parse_record(blocks)[0].kind, LineKind::User);
        let apierr = r#"{"type":"assistant","isApiErrorMessage":true,"message":{"content":[{"type":"text","text":"API Error: 529 overloaded"}]}}"#;
        assert_eq!(parse_record(apierr)[0].kind, LineKind::Error);
        for noise in [r#"{"type":"attachment","attachment":{}}"#, r#"{"type":"api-request","id":"x"}"#, r#"{"type":"last-prompt","lastPrompt":"x"}"#, "{}", "[]"] {
            assert!(parse_record(noise).is_empty(), "{noise}");
        }
        assert_eq!(parse_ts("2026-11-19T14:19:09.777Z"), Some(1_795_097_949_777));
        assert_eq!(parse_ts("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_ts("2000-02-29T00:00:00.5Z"), Some(951_782_400_500));
        assert_eq!(parse_ts("garbage"), None);
        assert_eq!(title_of_record(r#"{"type":"custom-title","customTitle":"Break","sessionId":"s"}"#).as_deref(), Some("Break"));
        assert_eq!(title_of_record(r#"{"type":"agent-name","agentName":"Splitter"}"#).as_deref(), Some("Splitter"));
        assert_eq!(title_of_record(r#"{"type":"user"}"#), None);
    }

    // ---- box 14: tailer
    #[test]
    fn plain_english_for_every_tool_and_result() {
        let p = |name: &str, input: &str| plain_tool(name, Json::parse(input).as_ref());
        assert_eq!(p("Read", r#"{"file_path":"C:\\a\\main.rs"}"#), "Reads the file main.rs");
        assert_eq!(p("Edit", r#"{"file_path":"/x/y/logs.rs"}"#), "Edits the file logs.rs");
        assert_eq!(p("Write", r#"{"file_path":"/x/new.txt"}"#), "Writes the file new.txt");
        assert_eq!(p("Bash", r#"{"command":"cargo test","description":"Run the tests"}"#), "Runs a command: Run the tests");
        assert_eq!(p("Bash", r#"{"command":"ls"}"#), "Runs a command in the terminal");
        assert_eq!(p("Grep", r#"{"pattern":"fn main"}"#), "Searches the code for \"fn main\"");
        assert_eq!(p("Glob", r#"{"pattern":"**/*.rs"}"#), "Looks for files matching **/*.rs");
        assert_eq!(p("WebFetch", r#"{"url":"https://example.com/a"}"#), "Opens the web page example.com/a");
        assert_eq!(p("WebSearch", r#"{"query":"rust egui"}"#), "Searches the web for \"rust egui\"");
        assert_eq!(p("Agent", r#"{"description":"find callers"}"#), "Starts a helper: find callers");
        assert_eq!(p("mcp__serena__find_symbol", "{}"), "Uses a helper tool");
        assert_eq!(p("Weird", "{}"), "Uses a tool");
        assert_eq!(p("Read", "{}"), "Reads the file");
        assert_eq!(plain_result("", false), "Done, nothing was printed");
        assert_eq!(plain_result("a\nb\nc", false), "Done, got 3 lines back");
        assert_eq!(plain_result("boom\nmore", true), "It failed: boom");
        // wired into the parser: tool call and result lines both carry a sentence
        let call = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/b.rs"}}]}}"#;
        assert_eq!(parse_record(call)[0].plain, "Reads the file b.rs");
        let res = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x\ny"}]}}"#;
        assert_eq!(parse_record(res)[0].plain, "Done, got 2 lines back");
    }

    // ---- box 15: one sentence per common tool
    #[test]
    fn plain_tool_sentences() {
        let p = |name: &str, input: &str| plain_tool(name, Json::parse(input).as_ref());
        assert_eq!(p("Bash", r#"{"command":"ls","description":"List files"}"#), "Runs a command: List files");
        assert_eq!(p("PowerShell", r#"{"command":"dir","description":"Show folder"}"#), "Runs a command: Show folder");
        assert_eq!(p("Read", r#"{"file_path":"/a/notes.md"}"#), "Reads the file notes.md");
        assert_eq!(p("Edit", r#"{"file_path":"/a/notes.md"}"#), "Edits the file notes.md");
        assert_eq!(p("Write", r#"{"file_path":"/a/new.md"}"#), "Writes the file new.md");
        assert_eq!(p("Grep", r#"{"pattern":"TODO"}"#), "Searches the code for \"TODO\"");
        assert_eq!(p("Glob", r#"{"pattern":"*.md"}"#), "Looks for files matching *.md");
        assert_eq!(p("Agent", r#"{"description":"check logs"}"#), "Starts a helper: check logs");
        assert_eq!(p("Task", r#"{"description":"check logs"}"#), "Starts a helper: check logs");
        assert_eq!(p("WebSearch", r#"{"query":"egui"}"#), "Searches the web for \"egui\"");
        assert_eq!(p("WebFetch", r#"{"url":"https://example.com"}"#), "Opens the web page example.com");
        assert_eq!(p("TodoWrite", "{}"), "Updates its to-do list");
        assert_eq!(p("Skill", r#"{"skill":"review"}"#), "Starts the skill review");
        assert_eq!(p("mcp__serena__find_symbol", "{}"), "Uses a helper tool");
        assert_eq!(p("Frobnicate", "{}"), "Uses a tool");
        assert_eq!(p("mcp__x__y", "{}"), "Uses a helper tool");
        assert!(!p("mcp__serena__find_symbol", "{}").contains("mcp__"));
        assert!(!p("Frobnicate", "{}").contains("Frobnicate"));
    }

    // ---- failure text: plain reason, paths cut to file names, never raw JSON/stack
    #[test]
    fn plain_failure_trims_paths() {
        assert_eq!(
            plain_result(r"C:\Users\Admin\AppData\Roaming\app\x.json: access denied", true),
            "It failed: x.json: access denied"
        );
        assert_eq!(plain_result("/home/u/build/out.log not found", true), "It failed: out.log not found");
        assert_eq!(plain_result("    at foo (x.js:1)\nreal reason", true), "It failed: real reason");
        assert_eq!(plain_result("{\"error\":1}", true), "It failed");
        assert_eq!(plain_result("boom\nmore", true), "It failed: boom");
        let long = format!("boom {}", "a".repeat(200));
        let out = plain_result(&long, true);
        assert_eq!(out.strip_prefix("It failed: ").unwrap().chars().count(), 90);
        let uni = format!("nope {}", "é".repeat(200));
        assert_eq!(plain_result(&uni, true).strip_prefix("It failed: ").unwrap().chars().count(), 90);
        assert_eq!(plain_result("", true), "It failed");
        assert_eq!(plain_result("   \n  \n", true), "It failed");
    }

    // ---- edge inputs: never panic, never empty
    #[test]
    fn plain_tool_edge_inputs_never_empty() {
        let long: String = "😀".repeat(5000);
        let desc = format!(r#"{{"command":"x","description":"{long}"}}"#);
        for (name, input) in [
            ("", None),
            ("", Json::parse("\"str\"").as_ref()),
            ("Read", Json::parse(r#"{"file_path":""}"#).as_ref()),
            ("Read", Json::parse(r#"{"file_path":"C:\\"}"#).as_ref()),
            ("Read", Json::parse(r#"{"file_path":"/"}"#).as_ref()),
            ("Bash", Json::parse(&desc).as_ref()),
            ("mcp__tail", None),
            ("bad\u{1}name", None),
        ] {
            let s = plain_tool(name, input);
            assert!(!s.is_empty(), "empty for {name:?}");
            assert!(std::str::from_utf8(s.as_bytes()).is_ok());
        }
        assert!(!plain_result("\r\n\r\n", false).is_empty());
        assert!(!plain_result("   ", true).is_empty());
    }

    // ---- box 16: command text never leads, description does
    #[test]
    fn plain_command_prefers_description() {
        let p = |name: &str, input: &str| plain_tool(name, Json::parse(input).as_ref());
        let with_desc = p("Bash", r#"{"command":"cargo test --bin breakguard","description":"Run the tests"}"#);
        assert_eq!(with_desc, "Runs a command: Run the tests");
        assert!(!with_desc.contains("cargo"));
        let no_desc = p("PowerShell", r#"{"command":"Remove-Item -Recurse x"}"#);
        assert_eq!(no_desc, "Runs a command in the terminal");
        assert!(!no_desc.contains("Remove-Item"));
    }

    #[test]
    fn tailer_chunked_appends_yield_exact_lines_once() {
        let d = tmp("tail");
        let p = d.join("s.jsonl");
        std::fs::write(&p, "").unwrap();
        let mut t = Tailer::new(p.clone());
        assert!(t.poll().is_empty());
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        let all = "{\"a\":1}\n{\"b\":\"\u{5e9}\u{5dc}\u{5d5}\u{5dd}\"}\r\n{\"c\":3}\nlast\n";
        let bytes = all.as_bytes();
        let mut got: Vec<String> = Vec::new();
        for chunk in bytes.chunks(5) {
            f.write_all(chunk).unwrap();
            f.flush().unwrap();
            got.extend(t.poll());
        }
        assert_eq!(got, ["{\"a\":1}", "{\"b\":\"\u{5e9}\u{5dc}\u{5d5}\u{5dd}\"}", "{\"c\":3}", "last"], "exact lines, once, partials held back");
        assert!(t.poll().is_empty(), "nothing twice");
        f.write_all(b"partial no newline").unwrap();
        assert!(t.poll().is_empty());
        f.write_all(b" done\n").unwrap();
        assert_eq!(t.poll(), ["partial no newline done"]);
    }

    // ---- box 19: robust reader
    #[test]
    fn reader_handles_rotation_truncation_huge_lines_and_bad_json() {
        let d = tmp("robust");
        let p = d.join("r.jsonl");
        std::fs::write(&p, "one\ntwo\n").unwrap();
        let mut t = Tailer::new(p.clone());
        assert_eq!(t.poll(), ["one", "two"]);
        // truncation: file rewritten shorter
        std::fs::write(&p, "x\n").unwrap();
        assert_eq!(t.poll(), ["x"], "truncated file read again from the top");
        // rotation: replaced by a longer, different file
        std::fs::remove_file(&p).unwrap();
        std::fs::write(&p, "new file first line\nsecond\nthird\n").unwrap();
        assert_eq!(t.poll(), ["new file first line", "second", "third"], "rotated file restarts");
        // huge line skipped whole, following lines survive
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        let huge = vec![b'h'; MAX_LINE_BYTES + 10_000];
        f.write_all(&huge).unwrap();
        f.write_all(b"\nafter huge\n").unwrap();
        f.flush().unwrap();
        let mut got = Vec::new();
        for _ in 0..8 {
            got.extend(t.poll());
        }
        assert_eq!(got, ["after huge"], "{} lines", got.len());
        assert_eq!(t.skipped_huge, 1);
        // bad json skipped, good ones parsed
        let recs = ["{not json", "\u{0}\u{1}", r#"{"type":"user","message":{"content":"ok"}}"#, "}{", ""];
        let n: usize = recs.iter().map(|r| parse_record(r).len()).sum();
        assert_eq!(n, 1);
        // missing file: no panic, no lines
        assert!(Tailer::new(d.join("nope.jsonl")).poll().is_empty());
    }

    // ---- box 15: names
    #[test]
    fn name_resolution_order() {
        assert_eq!(resolve_name(Some("Break"), "F:\\Downloads", "65e6eba6-fa0e"), "Break");
        assert_eq!(resolve_name(Some("  "), "F:\\study\\projects\\breakguard", "65e6eba6-fa0e"), "breakguard");
        assert_eq!(resolve_name(None, "", "65e6eba6-fa0e-4991"), "65e6eba6");
        assert_eq!(resolve_name(Some("\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{1f600}"), "", "x"), "\u{5e9}\u{5dc}\u{5d5}\u{5dd} \u{1f600}");
    }

    /// Manual: cargo test --release --bin breakguard -- --ignored live_names --nocapture
    /// Every live agent: the name Claude shows (desktop title / app name) vs the name resolved from its transcript.
    #[test]
    #[ignore]
    fn live_names() {
        let roots = crate::sessions::roots_with(None);
        let agents = crate::agents::discover(&roots, &crate::agents::process_alive);
        let mut compared = 0;
        for a in &agents {
            let Ok(rd) = std::fs::read_dir(&roots.projects) else { continue };
            let Some(tp) = rd.flatten().map(|d| d.path().join(format!("{}.jsonl", a.session_id))).find(|p| p.exists()) else { continue };
            let text = std::fs::read_to_string(&tp).unwrap_or_default();
            let title = text.lines().rev().find_map(title_of_record);
            let name = resolve_name(title.as_deref(), &a.cwd, &a.session_id);
            println!("LIVENAME|{}|shown={}|resolved={}", a.pid, a.name, name);
            if title.is_some() {
                compared += 1;
                assert_eq!(name, a.name, "pid {}", a.pid);
            }
        }
        println!("LIVENAME compared={compared} of {}", agents.len());
        assert!(compared > 0);
    }

    // ---- hub fixtures
    fn fixture(name: &str) -> (PathBuf, crate::sessions::Roots) {
        let d = tmp(name);
        std::fs::create_dir_all(d.join("sessions")).unwrap();
        std::fs::create_dir_all(d.join("projects").join("p")).unwrap();
        let r = crate::sessions::roots_with(Some(&d));
        (d, r)
    }
    fn add_session(d: &Path, pid: u32, sid: &str, cwd: &str, lines: &str) {
        let cwd = cwd.replace('\\', "\\\\");
        std::fs::write(d.join("sessions").join(format!("{pid}.json")), format!(r#"{{"pid":{pid},"sessionId":"{sid}","cwd":"{cwd}","startedAt":1}}"#)).unwrap();
        std::fs::write(d.join("projects").join("p").join(format!("{sid}.jsonl")), lines).unwrap();
    }
    fn all_alive() -> AliveBox {
        Box::new(|_, _| true)
    }
    fn asst(text: &str) -> String {
        format!("{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{text}\"}}]}}}}\n")
    }

    // ---- box 17: refresh latency
    #[test]
    fn appended_line_reaches_the_model_within_500ms() {
        let (d, r) = fixture("latency");
        add_session(&d, 4242, "sid-1", "C:\\w\\proj", &asst("first"));
        let mut hub = LogHub::new(r, all_alive());
        hub.tick(true, 1000);
        assert_eq!(hub.store.logs.len(), 1);
        assert_eq!(hub.store.logs[0].len(), 1);
        let tp = d.join("projects").join("p").join("sid-1.jsonl");
        let mut f = std::fs::OpenOptions::new().append(true).open(&tp).unwrap();
        let t0 = std::time::Instant::now();
        f.write_all(asst("live line").as_bytes()).unwrap();
        f.flush().unwrap();
        // the real loop polls every 250 ms; emulate that cadence
        let mut seen = None;
        for _ in 0..10 {
            hub.tick(false, 2000);
            if hub.store.logs[0].last().is_some_and(|l| l.text == "live line") {
                seen = Some(t0.elapsed());
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let el = seen.expect("line never arrived");
        assert!(el < std::time::Duration::from_millis(500), "latency {el:?}");
        assert_eq!(hub.store.logs[0].state, LogState::Running);
    }

    // ---- run 517e box 8: live tail through the real background loop
    #[test]
    fn live_tail_thread_shows_every_appended_line_within_1s() {
        let (d, r) = fixture("livetail");
        add_session(&d, 4343, "sid-live", "C:\\w\\live", &asst("history"));
        let wanted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let hub = spawn_hub_with(eframe::egui::Context::default(), wanted.clone(), LogHub::new(r, all_alive()));
        let wait = |pred: &dyn Fn(&LogHub) -> bool| {
            let t0 = std::time::Instant::now();
            while t0.elapsed() < std::time::Duration::from_secs(3) {
                if pred(&hub.lock().unwrap()) {
                    return Some(t0.elapsed());
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            None
        };
        wait(&|h| h.store.logs.len() == 1 && h.store.logs[0].len() == 1).expect("session card with history appears");
        let tp = d.join("projects").join("p").join("sid-live.jsonl");
        let mut f = std::fs::OpenOptions::new().append(true).open(&tp).unwrap();
        let mut worst = std::time::Duration::ZERO;
        for round in 0..5usize {
            // assistant text + tool call + tool result, written in two partial chunks
            let call = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test","description":"Run tests ROUND"}}]}}"#.replace("ROUND", &round.to_string());
            let res = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok ROUND","is_error":false}]}}"#.replace("ROUND", &round.to_string());
            let batch = format!("{}{call}\n{res}\n", asst(&format!("text {round}")));
            let (a, b) = batch.split_at(batch.len() / 2);
            f.write_all(a.as_bytes()).unwrap();
            f.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(30));
            let t_full = std::time::Instant::now();
            f.write_all(b.as_bytes()).unwrap();
            f.flush().unwrap();
            let want = 1 + 3 * (round + 1);
            wait(&|h| h.store.logs[0].len() == want).unwrap_or_else(|| panic!("round {round}: lines missing, have {}", hub.lock().unwrap().store.logs[0].len()));
            worst = worst.max(t_full.elapsed());
        }
        assert!(worst < std::time::Duration::from_secs(1), "worst latency {worst:?}");
        let h = hub.lock().unwrap();
        let got: Vec<(LineKind, String)> = h.store.logs[0].iter().map(|l| (l.kind, l.text.clone())).collect();
        let mut exp = vec![(LineKind::Assistant, "history".to_string())];
        for r in 0..5 {
            exp.push((LineKind::Assistant, format!("text {r}")));
            exp.push((LineKind::ToolCall, format!("Bash: Run tests {r}")));
            exp.push((LineKind::ToolResult, format!("ok {r}")));
        }
        assert_eq!(got, exp, "every line once, in order, no misses");
        assert_eq!(h.store.logs[0].state, LogState::Running);
        println!("LIVETAIL lines={} worst_latency_ms={}", got.len(), worst.as_millis());
        wanted.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    // ---- box 21: live discovery
    #[test]
    fn sessions_appear_and_disappear_without_restart() {
        let (d, r) = fixture("discover");
        let mut hub = LogHub::new(r, all_alive());
        hub.finished_keep_ms = 0;
        hub.tick(true, 0);
        assert!(hub.store.logs.is_empty());
        add_session(&d, 100, "a", "C:\\w\\alpha", &asst("hello a"));
        add_session(&d, 200, "b", "C:\\w\\beta", &asst("hello b"));
        hub.tick(true, 1000);
        let names: Vec<&str> = hub.store.logs.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.contains(&"alpha") && names.contains(&"beta"));
        std::fs::remove_file(d.join("sessions").join("100.json")).unwrap();
        hub.tick(true, 2000);
        assert_eq!(hub.store.logs.len(), 1, "removed session leaves the model");
        assert_eq!(hub.store.logs[0].id, "b");
        add_session(&d, 300, "c", "C:\\w\\gamma", &asst("late"));
        hub.tick(true, 3000);
        assert_eq!(hub.store.logs.len(), 2, "new session appears");
        // an old title record in the transcript is not output and does not rename the lane
        let tp = d.join("projects").join("p").join("c.jsonl");
        std::fs::OpenOptions::new().append(true).open(&tp).unwrap().write_all(b"{\"type\":\"custom-title\",\"customTitle\":\"Stale\"}\n").unwrap();
        hub.tick(false, 3100);
        assert!(hub.store.logs.iter().any(|l| l.id == "c" && l.name == "gamma"));
        // a rename in Claude (the <pid>.json name) shows on the next scan
        std::fs::write(d.join("sessions").join("300.json"), r#"{"pid":300,"sessionId":"c","cwd":"C:\\w\\gamma","startedAt":1,"name":"Renamed"}"#).unwrap();
        hub.tick(true, 3200);
        assert!(hub.store.logs.iter().any(|l| l.id == "c" && l.name == "Renamed"));
        // an ended session turns Finished at once and stays for the default keep (3 hours), then goes
        let mut dflt = LogHub::new(crate::sessions::roots_with(Some(&d)), all_alive());
        dflt.tick(true, 10_000);
        assert!(dflt.store.logs.iter().any(|l| l.id == "c"));
        std::fs::remove_file(d.join("sessions").join("300.json")).unwrap();
        dflt.tick(true, 11_000);
        assert!(dflt.store.logs.iter().any(|l| l.id == "c" && l.state == LogState::Finished), "grace: one missed scan");
        dflt.tick(true, 14_000);
        assert!(dflt.store.logs.iter().any(|l| l.id == "c" && l.state == LogState::Finished), "still on the Finished tab 3 s later");
        assert_eq!(dflt.gone_since.get("c").copied(), Some(11_000), "finish time recorded for the Finished tab");
        dflt.tick(true, 11_000 + crate::ui::logic::tabs::FINISHED_KEEP_MS);
        assert!(dflt.store.logs.iter().all(|l| l.id != "c"), "dropped after the finished keep");
        // finished lane lingers for finished_keep_ms
        let mut keep = LogHub::new(crate::sessions::roots_with(Some(&d)), Box::new(|pid, _| pid != 200));
        keep.finished_keep_ms = 60_000;
        keep.tick(true, 0);
        assert!(keep.store.logs.iter().all(|l| l.id != "b"), "dead process never starts a lane");
    }

    // ---- box 22: ralp footer
    #[test]
    fn ralp_footer_from_fixture_run_dir() {
        let (d, r) = fixture("ralp");
        let run = d.join("run-1");
        std::fs::create_dir_all(&run).unwrap();
        let cl = run.join(crate::sessions::BOXFILE);
        std::fs::write(&cl, "# list\n- [x] 1. a\n- [x] 2. b\n- [ ] 3. c\n- [ ] 4. d\n").unwrap();
        std::fs::write(run.join("LEDGER.md"), "D1 | one\n\nD2 | two\nD3 | three\nD4 | four\n").unwrap();
        let f = read_ralp(&cl, 3).unwrap();
        assert_eq!((f.done, f.total), (2, 4));
        assert_eq!(f.ledger_tail, ["D2 | two", "D3 | three", "D4 | four"]);
        assert_eq!(f.run_dir, run);
        assert!(read_ralp(&run.join("missing.md"), 3).is_none());
        // a session whose transcript mentions the checklist path gets the footer
        let mention = format!("{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"see {}\"}}]}}}}\n", cl.to_string_lossy().replace('\\', "/"));
        add_session(&d, 500, "r1", "C:\\w\\ralp", &mention);
        let mut hub = LogHub::new(r, all_alive());
        hub.tick(true, 0);
        hub.tick(true, 1000);
        let foot = hub.ralp.get("r1").expect("footer for the session");
        assert_eq!((foot.done, foot.total), (2, 4));
    }

    // ---- box 16: state
    #[test]
    fn state_from_process_and_write_age() {
        assert_eq!(detect_state(true, 0), LogState::Running);
        assert_eq!(detect_state(true, RUNNING_WINDOW_SECS), LogState::Running);
        assert_eq!(detect_state(true, RUNNING_WINDOW_SECS + 1), LogState::Idle);
        assert_eq!(detect_state(false, 0), LogState::Finished);
        assert_eq!(detect_state(false, 99999), LogState::Finished);
    }
}
