//! Break-start action choice model (box 7).
//!
//! Each break can run one or more actions at break start; every action has its
//! own duration (`duration_secs`, 0 = for the whole break). The model persists as
//! plain text so it fits the existing key=value settings file:
//! `key:secs` for every action, `custom:secs:<escaped command>` for custom,
//! several actions joined with `;`.
#![allow(dead_code)]

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionKind {
    None,
    OverlayOnly,
    SleepMainMonitor,
    SleepSecondaryMonitors,
    SleepAllMonitors,
    Dim,
    LockWorkstation,
    MuteAudio,
    PauseMedia,
    MinimizeAllWindows,
    CustomCommand(String),
}

impl ActionKind {
    /// Every variant once (custom with an empty command), for pickers and tests.
    pub fn all() -> Vec<ActionKind> {
        vec![
            ActionKind::None,
            ActionKind::OverlayOnly,
            ActionKind::SleepMainMonitor,
            ActionKind::SleepSecondaryMonitors,
            ActionKind::SleepAllMonitors,
            ActionKind::Dim,
            ActionKind::LockWorkstation,
            ActionKind::MuteAudio,
            ActionKind::PauseMedia,
            ActionKind::MinimizeAllWindows,
            ActionKind::CustomCommand(String::new()),
        ]
    }

    pub fn key(&self) -> &'static str {
        match self {
            ActionKind::None => "none",
            ActionKind::OverlayOnly => "overlay",
            ActionKind::SleepMainMonitor => "sleep_main",
            ActionKind::SleepSecondaryMonitors => "sleep_secondary",
            ActionKind::SleepAllMonitors => "sleep_all",
            ActionKind::Dim => "dim",
            ActionKind::LockWorkstation => "lock",
            ActionKind::MuteAudio => "mute",
            ActionKind::PauseMedia => "pause_media",
            ActionKind::MinimizeAllWindows => "minimize_all",
            ActionKind::CustomCommand(_) => "custom",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ActionKind::None => "None",
            ActionKind::OverlayOnly => "Overlay only",
            ActionKind::SleepMainMonitor => "Sleep main monitor",
            ActionKind::SleepSecondaryMonitors => "Sleep secondary monitor(s)",
            ActionKind::SleepAllMonitors => "Sleep all monitors",
            ActionKind::Dim => "Dim",
            ActionKind::LockWorkstation => "Lock workstation",
            ActionKind::MuteAudio => "Mute audio",
            ActionKind::PauseMedia => "Pause media",
            ActionKind::MinimizeAllWindows => "Minimize all windows",
            ActionKind::CustomCommand(_) => "Custom command",
        }
    }

    fn from_key(key: &str, cmd: Option<String>) -> Option<ActionKind> {
        Some(match key {
            "none" => ActionKind::None,
            "overlay" => ActionKind::OverlayOnly,
            "sleep_main" => ActionKind::SleepMainMonitor,
            "sleep_secondary" => ActionKind::SleepSecondaryMonitors,
            "sleep_all" => ActionKind::SleepAllMonitors,
            "dim" => ActionKind::Dim,
            "lock" => ActionKind::LockWorkstation,
            "mute" => ActionKind::MuteAudio,
            "pause_media" => ActionKind::PauseMedia,
            "minimize_all" => ActionKind::MinimizeAllWindows,
            "custom" => ActionKind::CustomCommand(cmd.unwrap_or_default()),
            _ => return None,
        })
    }
}

/// Max per-action duration: 24 h.
pub const MAX_ACTION_SECS: u32 = 24 * 3600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakAction {
    pub kind: ActionKind,
    /// Seconds the action lasts from break start; 0 = the whole break.
    pub duration_secs: u32,
}

impl BreakAction {
    pub fn new(kind: ActionKind, duration_secs: u32) -> Self {
        BreakAction { kind, duration_secs: duration_secs.min(MAX_ACTION_SECS) }
    }

    pub fn to_text(&self) -> String {
        match &self.kind {
            ActionKind::CustomCommand(c) => format!("custom:{}:{}", self.duration_secs, escape(c)),
            k => format!("{}:{}", k.key(), self.duration_secs),
        }
    }

    pub fn from_text(s: &str) -> Option<BreakAction> {
        let mut it = s.trim().splitn(3, ':');
        let key = it.next()?;
        let secs = match it.next() {
            Some(t) => t.trim().parse::<u32>().ok()?,
            None => 0,
        };
        let cmd = it.next().map(unescape);
        Some(BreakAction::new(ActionKind::from_key(key, cmd)?, secs))
    }
}

/// Serialize a list of actions (settings value).
pub fn actions_to_text(list: &[BreakAction]) -> String {
    list.iter().map(|a| a.to_text()).collect::<Vec<_>>().join(";")
}

/// Parse a settings value; unknown entries are skipped. Empty -> `[None]`.
pub fn actions_from_text(s: &str) -> Vec<BreakAction> {
    let v: Vec<BreakAction> = s.split(';').filter(|p| !p.trim().is_empty()).filter_map(BreakAction::from_text).collect();
    if v.is_empty() { vec![BreakAction::new(ActionKind::None, 0)] } else { v }
}

/// Plain-English "what happens" text for a break's chosen actions, shown next to the chips.
pub fn actions_preview(list: &[BreakAction]) -> String {
    let parts: Vec<String> = list
        .iter()
        .filter(|a| a.kind != ActionKind::None)
        .map(|a| {
            let what = match &a.kind {
                ActionKind::None => String::new(),
                ActionKind::OverlayOnly => "show the break screen".into(),
                ActionKind::SleepMainMonitor => "turn off the main screen".into(),
                ActionKind::SleepSecondaryMonitors => "turn off the other screens".into(),
                ActionKind::SleepAllMonitors => "turn off all screens".into(),
                ActionKind::Dim => "dim the screen".into(),
                ActionKind::LockWorkstation => "lock the computer".into(),
                ActionKind::MuteAudio => "mute the sound".into(),
                ActionKind::PauseMedia => "pause music and video".into(),
                ActionKind::MinimizeAllWindows => "minimize all windows".into(),
                ActionKind::CustomCommand(c) if c.trim().is_empty() => "run your own command".into(),
                ActionKind::CustomCommand(c) => format!("run your command \"{}\"", c.trim()),
            };
            if a.duration_secs == 0 {
                what
            } else if a.duration_secs % 60 == 0 {
                format!("{what} for {} min", a.duration_secs / 60)
            } else {
                format!("{what} for {} s", a.duration_secs)
            }
        })
        .collect();
    match parts.len() {
        0 => "At break start: nothing else happens.".to_string(),
        1 => format!("At break start: {}.", parts[0]),
        n => format!("At break start: {} and {}.", parts[..n - 1].join(", "), parts[n - 1]),
    }
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '%' => o.push_str("%25"),
            ';' => o.push_str("%3B"),
            '\n' => o.push_str("%0A"),
            '\r' => o.push_str("%0D"),
            c => o.push(c),
        }
    }
    o
}

fn unescape(s: &str) -> String {
    s.replace("%3B", ";").replace("%0A", "\n").replace("%0D", "\r").replace("%25", "%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_preview_plain_english() {
        assert_eq!(actions_preview(&actions_from_text("")), "At break start: nothing else happens.");
        let l = actions_from_text("dim:0;mute:0;lock:90");
        assert_eq!(actions_preview(&l), "At break start: dim the screen, mute the sound and lock the computer for 90 s.");
        assert_eq!(actions_preview(&actions_from_text("pause_media:120")), "At break start: pause music and video for 2 min.");
        let c = actions_preview(&actions_from_text("custom:0:calc"));
        assert!(c.contains("run your command \"calc\""), "{c}");
        for k in ActionKind::all() {
            let p = actions_preview(&[BreakAction::new(k, 0)]);
            assert!(!p.contains('_') && p.ends_with('.'), "{p}");
        }
    }

    #[test]
    fn every_variant_roundtrips() {
        for (i, k) in ActionKind::all().into_iter().enumerate() {
            let k = match k {
                ActionKind::CustomCommand(_) => ActionKind::CustomCommand("cmd /c echo a;b %X% & x:y".into()),
                k => k,
            };
            let a = BreakAction::new(k, (i as u32) * 30);
            let t = a.to_text();
            assert_eq!(BreakAction::from_text(&t), Some(a.clone()), "variant {t}");
        }
    }

    #[test]
    fn list_roundtrips_with_per_action_duration() {
        let list = vec![
            BreakAction::new(ActionKind::SleepSecondaryMonitors, 300),
            BreakAction::new(ActionKind::MuteAudio, 0),
            BreakAction::new(ActionKind::CustomCommand("notepad.exe; calc".into()), 60),
        ];
        let t = actions_to_text(&list);
        assert_eq!(actions_from_text(&t), list);
        assert_eq!(ActionKind::all().len(), 11);
    }

    #[test]
    fn bad_input_is_safe() {
        assert_eq!(actions_from_text(""), vec![BreakAction::new(ActionKind::None, 0)]);
        assert_eq!(BreakAction::from_text("bogus:5"), None);
        assert_eq!(BreakAction::from_text("dim:abc"), None);
        assert_eq!(BreakAction::from_text("lock").unwrap().duration_secs, 0);
        assert_eq!(BreakAction::from_text("dim:999999999").unwrap().duration_secs, MAX_ACTION_SECS);
    }
}
