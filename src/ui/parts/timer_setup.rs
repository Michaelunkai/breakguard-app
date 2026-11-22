//! "Timer setup" column drawn right of the Dashboard countdown ring:
//! work length, break length, what happens during the break, and extra break options.
use crate::break_action::{actions_from_text, actions_to_text, ActionKind, BreakAction, MAX_ACTION_SECS};
use crate::engine::{Settings, BREAK_MAX_SECS, MAX_WORK_MIN};
use crate::ui::{theme, widgets, App};
use eframe::egui::{self, RichText, Ui};

/// Smallest hero width (px) that has room for the setup column beside the ring.
pub const SIDE_MIN_W: f32 = 720.0;

pub fn work_len_chips() -> [u32; 7] {
    [15, 20, 25, 30, 45, 60, 90]
}

/// Break length quick picks in seconds: 20 s .. 15 min.
pub fn break_len_chips() -> [u32; 7] {
    [20, 30, 60, 120, 300, 600, 900]
}

pub fn warn_chips() -> [u32; 4] {
    [0, 10, 30, 60]
}

/// Setup goes beside the ring when the hero is wide enough, else below it.
pub fn hero_controls_fit(avail_w: f32) -> bool {
    avail_w.is_finite() && avail_w >= SIDE_MIN_W
}

/// (name, work minutes, break seconds)
pub const PRESETS: [(&str, u32, u32); 4] = [("Eyes 20-20-20", 20, 20), ("Pomodoro 25/5", 25, 300), ("Deep work 50/10", 50, 600), ("Long 90/15", 90, 900)];

pub fn apply_preset(s: &mut Settings, i: usize) -> bool {
    match PRESETS.get(i) {
        Some(&(_, w, b)) => {
            s.work_min = w;
            s.break_secs = b;
            true
        }
        None => false,
    }
}

pub fn reset_defaults(s: &mut Settings) {
    s.work_min = 20;
    s.break_secs = 20;
    s.warn_secs = 10;
    s.break_actions = String::new();
}

pub fn clamp_work(v: i64) -> u32 {
    v.clamp(1, MAX_WORK_MIN as i64) as u32
}

pub fn split_min_sec(secs: u32) -> (u32, u32) {
    (secs / 60, secs % 60)
}

pub fn clamp_break(v: i64) -> u32 {
    v.clamp(5, BREAK_MAX_SECS as i64) as u32
}

/// Parse typed text ("25", "1h30", "2:15") into minutes, clamped; None when it is not a duration.
pub fn parse_work_text(t: &str) -> Option<u32> {
    crate::engine::parse_duration(t).map(|secs| clamp_work((secs as i64 + 30) / 60))
}

pub fn parse_break_text(t: &str) -> Option<u32> {
    crate::engine::parse_duration(t).map(|secs| clamp_break(secs as i64))
}

fn dur(secs: u32) -> String {
    if secs % 3600 == 0 && secs >= 3600 {
        format!("{} h", secs / 3600)
    } else if secs % 60 == 0 {
        format!("{} min", secs / 60)
    } else if secs > 60 {
        format!("{} min {} s", secs / 60, secs % 60)
    } else {
        format!("{secs} s")
    }
}

pub fn actions_list(s: &Settings) -> Vec<BreakAction> {
    actions_from_text(&s.break_actions)
}

/// Plain-English list of what happens when a break starts.
pub fn actions_preview(s: &Settings) -> String {
    let names: Vec<&str> = actions_list(s).iter().filter(|a| !matches!(a.kind, ActionKind::None)).map(|a| a.kind.label()).collect();
    if names.is_empty() {
        "Nothing extra happens during the break.".to_string()
    } else {
        format!("When a break starts: {}.", names.join(", ").to_lowercase())
    }
}

/// One line: "Work 25 min, break 2 min, then: dim, mute audio".
pub fn setup_summary(s: &Settings) -> String {
    let names: Vec<String> = actions_list(s).iter().filter(|a| !matches!(a.kind, ActionKind::None)).map(|a| a.kind.label().to_lowercase()).collect();
    let tail = if names.is_empty() { String::new() } else { format!(", then: {}", names.join(", ")) };
    format!("Work {}, break {}{}", dur(s.work_min * 60), dur(s.break_secs), tail)
}

/// Toggle one action in the saved text (turning the first real action on removes "none").
pub fn toggle_action(s: &mut Settings, k: &ActionKind) {
    let mut list = actions_list(s);
    list.retain(|a| !matches!(a.kind, ActionKind::None));
    let same = |a: &ActionKind| std::mem::discriminant(a) == std::mem::discriminant(k);
    if let Some(i) = list.iter().position(|a| same(&a.kind)) {
        list.remove(i);
    } else {
        list.push(BreakAction::new(k.clone(), 0));
    }
    s.break_actions = if list.is_empty() { String::new() } else { actions_to_text(&list) };
}

pub fn action_on(s: &Settings, k: &ActionKind) -> bool {
    let same = |a: &ActionKind| std::mem::discriminant(a) == std::mem::discriminant(k);
    actions_list(s).iter().any(|a| same(&a.kind))
}

/// The column. Returns true when any setting changed (caller marks dirty).
pub fn show(app: &mut App, ui: &mut Ui) -> bool {
    let mut s = app.engine.settings().clone();
    let before = s.clone();
    ui.vertical(|ui| {
        ui.label(RichText::new("Timer setup").strong().size(15.0));
        ui.label(RichText::new(setup_summary(&s)).color(theme::pal().text_dim));
        ui.add_space(theme::SP_XS);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Quick start").color(theme::pal().text_dim));
            for (i, (name, _, _)) in PRESETS.iter().enumerate() {
                if widgets::chip(ui, name, false).on_hover_text("Sets work and break length together").clicked() {
                    apply_preset(&mut s, i);
                }
            }
            if widgets::ghost_button(ui, "Reset").on_hover_text("Back to 20 min work, 20 s break, nothing extra").clicked() {
                reset_defaults(&mut s);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Work").color(theme::pal().text_dim));
            for m in work_len_chips() {
                if widgets::chip(ui, &dur(m * 60), s.work_min == m).on_hover_text("Work this long before each break").clicked() {
                    s.work_min = m;
                }
            }
            let mut v = s.work_min;
            ui.add(egui::DragValue::new(&mut v).range(1..=MAX_WORK_MIN).speed(0.5).suffix(" min")).on_hover_text("Drag or type any length, 1 min to 24 h");
            s.work_min = clamp_work(v as i64);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Break").color(theme::pal().text_dim));
            for b in break_len_chips() {
                if widgets::chip(ui, &dur(b), s.break_secs == b).on_hover_text("Rest this long each break").clicked() {
                    s.break_secs = b;
                }
            }
            let (mut bm, mut bs) = split_min_sec(s.break_secs);
            ui.add(egui::DragValue::new(&mut bm).range(0..=240).speed(0.2).suffix(" min")).on_hover_text("Custom break minutes");
            ui.add(egui::DragValue::new(&mut bs).range(0..=59).speed(0.5).suffix(" sec")).on_hover_text("Custom break seconds");
            s.break_secs = clamp_break(bm as i64 * 60 + bs as i64);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Long break").color(theme::pal().text_dim));
            for b in [120u32, 300, 600, 900] {
                if widgets::chip(ui, &dur(b), s.long_break_secs == b).on_hover_text("Length of the longer break").clicked() {
                    s.long_break_secs = b;
                }
            }
            let (mut lm, mut ls) = split_min_sec(s.long_break_secs);
            ui.add(egui::DragValue::new(&mut lm).range(0..=240).speed(0.2).suffix(" min")).on_hover_text("Long break minutes");
            ui.add(egui::DragValue::new(&mut ls).range(0..=59).speed(0.5).suffix(" sec")).on_hover_text("Long break seconds");
            s.long_break_secs = clamp_break(lm as i64 * 60 + ls as i64);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Warn me").color(theme::pal().text_dim));
            for w in warn_chips() {
                let label = if w == 0 { "no warning".to_string() } else { format!("{w} s before") };
                if widgets::chip(ui, &label, s.warn_secs == w).on_hover_text("Heads-up before the break starts").clicked() {
                    s.warn_secs = w;
                }
            }
        });
        ui.label(RichText::new("During the break").color(theme::pal().text_dim));
        ui.horizontal_wrapped(|ui| {
            for k in ActionKind::all().into_iter().filter(|k| !matches!(k, ActionKind::None)) {
                if widgets::chip(ui, k.label(), action_on(&s, &k)).on_hover_text("Tap to turn on or off for every break").clicked() {
                    toggle_action(&mut s, &k);
                }
            }
        });
        ui.label(RichText::new(actions_preview(&s)).color(theme::pal().text_dim));
        ui.horizontal_wrapped(|ui| {
            widgets::toggle(ui, &mut s.allow_skip, "Allow skipping").on_hover_text("Lets you end a break early with the Skip button");
            widgets::toggle(ui, &mut s.sound, "Sound").on_hover_text("Play a short sound when a break starts and ends");
            widgets::toggle(ui, &mut s.auto_resume, "Auto-resume work").on_hover_text("Start the work timer by itself when a break ends. Off waits until you press resume");
            ui.label(RichText::new("Long break every").color(theme::pal().text_dim));
            ui.add(egui::DragValue::new(&mut s.long_break_every).range(0..=50).suffix(" breaks")).on_hover_text("Every this many breaks, take a longer one. 0 means never");
        });
    });
    s.work_min = clamp_work(s.work_min as i64);
    s.break_secs = clamp_break(s.break_secs as i64);
    s.warn_secs = s.warn_secs.min(120);
    let _ = MAX_ACTION_SECS;
    if s != before {
        *app.engine.settings_mut() = s;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_tooltips_present() {
        let src = include_str!("timer_setup.rs");
        let body = src.split("pub fn show(").nth(1).unwrap().split("#[cfg(test)]").next().unwrap();
        let mut n = 0;
        for l in body.lines() {
            let l = l.trim();
            if l.contains("widgets::chip(") || l.contains("widgets::ghost_button(") || l.contains("widgets::toggle(") || l.contains("DragValue::new(") {
                n += 1;
                assert!(l.contains(".on_hover_text(\""), "no tooltip: {l}");
                let tip = l.split(".on_hover_text(\"").nth(1).unwrap().split('"').next().unwrap();
                assert!(tip.len() >= 10 && !tip.contains("::") && !tip.contains('_'), "bad tooltip: {tip}");
            }
        }
        assert!(n >= 11, "only {n} controls found");
    }

    #[test]
    fn hero_controls_chips_and_fit() {
        assert_eq!(work_len_chips()[0], 15);
        assert_eq!(break_len_chips()[0], 20);
        assert!(work_len_chips().windows(2).all(|w| w[0] < w[1]));
        assert!(break_len_chips().windows(2).all(|w| w[0] < w[1]));
        assert!(hero_controls_fit(1600.0));
        assert!(!hero_controls_fit(719.0));
        assert!(!hero_controls_fit(f32::NAN));
    }

    #[test]
    fn hero_presets_apply_values() {
        let mut s = Settings::default();
        assert!(apply_preset(&mut s, 1));
        assert_eq!((s.work_min, s.break_secs), (25, 300));
        assert!(!apply_preset(&mut s, 99));
        assert_eq!((s.work_min, s.break_secs), (25, 300));
    }

    #[test]
    fn hero_reset_defaults() {
        let mut s = Settings::default();
        s.work_min = 90;
        s.break_secs = 900;
        toggle_action(&mut s, &ActionKind::Dim);
        reset_defaults(&mut s);
        assert_eq!((s.work_min, s.break_secs, s.break_actions.as_str()), (20, 20, ""));
    }

    #[test]
    fn hero_inputs_clamped() {
        assert_eq!(clamp_work(0), 1);
        assert_eq!(clamp_work(-5), 1);
        assert_eq!(clamp_work(i64::MAX), MAX_WORK_MIN);
        assert_eq!(clamp_break(0), 5);
        assert_eq!(clamp_break(i64::MIN), 5);
        assert_eq!(clamp_break(i64::MAX), BREAK_MAX_SECS);
        assert_eq!(parse_work_text("1h30m"), Some(90));
        assert_eq!(parse_work_text("abc"), None);
        assert_eq!(parse_break_text("45s"), Some(45));
    }

    #[test]
    fn hero_work_len_writes_setting() {
        let mut s = Settings::default();
        s.work_min = clamp_work(45);
        assert_eq!(s.work_min, 45);
    }

    #[test]
    fn hero_break_len_writes_setting() {
        let mut s = Settings::default();
        s.break_secs = clamp_break(120);
        assert_eq!(s.break_secs, 120);
        let (m, sec) = split_min_sec(95);
        s.break_secs = clamp_break(m as i64 * 60 + sec as i64);
        assert_eq!(s.break_secs, 95);
        s.long_break_secs = clamp_break(600);
        let r = Settings::parse(&s.serialize());
        assert_eq!((r.break_secs, r.long_break_secs), (95, 600));
    }

    #[test]
    fn hero_break_actions_toggle() {
        let mut s = Settings::default();
        toggle_action(&mut s, &ActionKind::Dim);
        toggle_action(&mut s, &ActionKind::MuteAudio);
        assert!(action_on(&s, &ActionKind::Dim) && action_on(&s, &ActionKind::MuteAudio));
        assert!(!action_on(&s, &ActionKind::LockWorkstation));
        let text = s.break_actions.clone();
        assert_eq!(actions_to_text(&actions_from_text(&text)), text, "roundtrip");
        toggle_action(&mut s, &ActionKind::Dim);
        assert!(!action_on(&s, &ActionKind::Dim));
        toggle_action(&mut s, &ActionKind::MuteAudio);
        assert_eq!(s.break_actions, "");
        // every kind (custom included) can be turned on
        for k in ActionKind::all().into_iter().filter(|k| !matches!(k, ActionKind::None)) {
            let mut t = Settings::default();
            toggle_action(&mut t, &k);
            assert!(action_on(&t, &k), "{}", k.label());
        }
    }

    #[test]
    fn setup_summary_text() {
        let mut s = Settings::default();
        s.work_min = 25;
        s.break_secs = 120;
        assert_eq!(setup_summary(&s), "Work 25 min, break 2 min");
        toggle_action(&mut s, &ActionKind::Dim);
        toggle_action(&mut s, &ActionKind::MuteAudio);
        assert_eq!(setup_summary(&s), "Work 25 min, break 2 min, then: dim, mute audio");
    }

    #[test]
    fn actions_preview_plain_english() {
        let mut s = Settings::default();
        assert_eq!(actions_preview(&s), "Nothing extra happens during the break.");
        toggle_action(&mut s, &ActionKind::LockWorkstation);
        assert_eq!(actions_preview(&s), "When a break starts: lock workstation.");
    }

    #[test]
    fn tsx_work_chip_values() {
        assert_eq!(work_len_chips(), [15, 20, 25, 30, 45, 60, 90]);
    }

    #[test]
    fn tsx_break_chip_values() {
        assert_eq!(break_len_chips(), [20, 30, 60, 120, 300, 600, 900]);
    }

    #[test]
    fn tsx_fit_boundary_exact() {
        assert!(hero_controls_fit(SIDE_MIN_W));
        assert!(!hero_controls_fit(SIDE_MIN_W - 0.5));
        assert!(!hero_controls_fit(-1.0));
    }

    #[test]
    fn tsx_long_preset_values() {
        let mut s = Settings::default();
        assert!(apply_preset(&mut s, 3));
        assert_eq!((s.work_min, s.break_secs), (90, 900));
    }

    #[test]
    fn tsx_reset_idempotent() {
        let mut s = Settings::default();
        s.work_min = 60;
        reset_defaults(&mut s);
        let once = (s.work_min, s.break_secs, s.warn_secs, s.break_actions.clone());
        reset_defaults(&mut s);
        assert_eq!(once, (s.work_min, s.break_secs, s.warn_secs, s.break_actions.clone()));
    }

    #[test]
    fn tsx_summary_seconds_format() {
        let mut s = Settings::default();
        s.work_min = 60;
        s.break_secs = 95;
        assert_eq!(setup_summary(&s), "Work 1 h, break 1 min 35 s");
        s.break_secs = 20;
        assert_eq!(setup_summary(&s), "Work 1 h, break 20 s");
    }

    #[test]
    fn tsx_toggle_twice_clears() {
        let mut s = Settings::default();
        toggle_action(&mut s, &ActionKind::Dim);
        toggle_action(&mut s, &ActionKind::Dim);
        assert_eq!(s.break_actions, "");
        assert!(!action_on(&s, &ActionKind::Dim));
    }

    #[test]
    fn tsx_parse_colon_and_hours() {
        assert_eq!(parse_work_text("2:15"), Some(135));
        assert_eq!(parse_work_text("2h"), Some(120));
        assert_eq!(parse_work_text(""), None);
    }

    #[test]
    fn tsx_warn_chips_sorted_unique() {
        let w = warn_chips();
        assert_eq!(w[0], 0);
        assert!(w.windows(2).all(|p| p[0] < p[1]));
    }

    #[test]
    fn tsx_bad_preset_keeps_settings() {
        let mut s = Settings::default();
        let (w, b) = (s.work_min, s.break_secs);
        assert!(!apply_preset(&mut s, PRESETS.len()));
        assert_eq!((s.work_min, s.break_secs), (w, b));
    }
}
