//! One file per screen. OWNER: Frontend Engineer.
pub mod about;
pub mod break_duration;
pub mod breaks;
pub mod dashboard;
pub mod lanes;
pub mod log;
pub mod monitors;
pub mod night;
pub mod schedule;
pub mod sessions;
pub mod settings;
pub mod stats;

use eframe::egui::{self, Ui};

pub fn fmt_mmss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Local wall-clock HH:MM for a unix time (uses the offset between local clock and unix now).
pub fn fmt_hm(unix: i64) -> String {
    let now = crate::sys::local_now();
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let local_secs = now.hour as i64 * 3600 + now.minute as i64 * 60 + now.second as i64;
    let t = (local_secs + (unix - now_unix)).rem_euclid(86400);
    format!("{:02}:{:02}", t / 3600, (t % 3600) / 60)
}

/// "20 s", "5 min", "1 h 30 min" for a length in seconds.
pub fn fmt_dur(secs: u32) -> String {
    if secs < 60 {
        format!("{secs} s")
    } else if secs.is_multiple_of(60) && secs < 3600 {
        format!("{} min", secs / 60)
    } else if secs >= 3600 {
        let (h, m) = (secs / 3600, (secs % 3600) / 60);
        if m == 0 { format!("{h} h") } else { format!("{h} h {m} min") }
    } else {
        format!("{}:{:02} min", secs / 60, secs % 60)
    }
}

pub fn title(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.heading(text);
    ui.add_space(8.0);
}

/// Truncated label with the full text as a tooltip.
pub fn short_label(ui: &mut Ui, text: &str, max_chars: usize) {
    let n = text.chars().count();
    if n > max_chars {
        let s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        ui.label(format!("{s}...")).on_hover_text(text);
    } else {
        ui.label(text);
    }
}

/// Quick lengths offered under every screen-off length editor (label, seconds).
pub const OFF_CHIPS: [(&str, u32); 12] = [
    ("30 s", 30), ("1 min", 60), ("5 min", 300), ("10 min", 600), ("15 min", 900), ("20 min", 1200),
    ("30 min", 1800), ("45 min", 2700), ("1 h", 3600), ("1 h 30 min", 5400), ("2 h", 7200), ("4 h", 14400),
];

/// Exact screen-off length editor: hours / minutes / seconds boxes, a free-text box ("1h30m", "90",
/// "1:30:00", "45s") and quick chips. `id` keeps each editor's typed text apart. Returns true when changed.
pub fn duration_edit(ui: &mut Ui, id: &str, secs: &mut u32) -> bool {
    duration_edit_in(ui, id, secs, 1, &OFF_CHIPS)
}

/// Quick start delays offered by the "Start in" editor (label, seconds); 0 = start right now.
pub const DELAY_CHIPS: [(&str, u32); 10] = [
    ("Now", 0), ("1 min", 60), ("5 min", 300), ("10 min", 600), ("15 min", 900), ("30 min", 1800),
    ("45 min", 2700), ("1 h", 3600), ("2 h", 7200), ("4 h", 14400),
];

/// "Start in" editor: how long from now a screen-off begins (0 = now, up to 24 h). Same boxes, free text
/// and chips as the length editor. Returns true when changed.
pub fn delay_edit(ui: &mut Ui, id: &str, secs: &mut u32) -> bool {
    duration_edit_in(ui, id, secs, 0, &DELAY_CHIPS)
}

fn duration_edit_in(ui: &mut Ui, id: &str, secs: &mut u32, min: u32, chips: &[(&str, u32)]) -> bool {
    use crate::engine::{parse_duration, MAX_OFF_SECS};
    let parse = |t: &str| -> Option<u32> {
        if min == 0 && matches!(t.trim().to_ascii_lowercase().as_str(), "0" | "now") {
            Some(0)
        } else {
            parse_duration(t)
        }
    };
    let mut changed = false;
    let (mut h, mut m, mut s) = (*secs / 3600, *secs / 60 % 60, *secs % 60);
    ui.horizontal_wrapped(|ui| {
        let hint = "Drag, or double-click to type";
        let a = ui.add(egui::DragValue::new(&mut h).range(0..=24).suffix(" h")).on_hover_text(format!("Hours. {hint}")).changed();
        let b = ui.add(egui::DragValue::new(&mut m).range(0..=59).suffix(" min")).on_hover_text(format!("Minutes. {hint}")).changed();
        let c = ui.add(egui::DragValue::new(&mut s).range(0..=59).suffix(" s")).on_hover_text(format!("Seconds. {hint}")).changed();
        if a || b || c {
            *secs = (h * 3600 + m * 60 + s).clamp(min, MAX_OFF_SECS);
            changed = true;
        }
        ui.label(egui::RichText::new("or type").color(crate::ui::theme::pal().text_dim));
        let tid = egui::Id::new(("off_len_text", id));
        let mut text: String = ui.data_mut(|d| d.get_temp(tid)).unwrap_or_default();
        let r = ui
            .add(egui::TextEdit::singleline(&mut text).hint_text("1h30m, 90, 1:30:00, 45s").desired_width(170.0))
            .on_hover_text(if min == 0 { "Any time from now up to 24 hours. A plain number means minutes; 0 or now = start immediately." } else { "Any length from 1 second to 24 hours. A plain number means minutes." });
        if r.changed() {
            if let Some(v) = parse(&text) {
                *secs = v;
                changed = true;
            }
            ui.data_mut(|d| d.insert_temp(tid, text.clone()));
        }
        if !text.trim().is_empty() && parse(&text).is_none() {
            let col = self::log::line_color(self::log::LineKind::Error, crate::ui::theme::is_light());
            ui.label(egui::RichText::new(if min == 0 { "Not a valid time (0 to 24 h)" } else { "Not a valid length (1 s to 24 h)" }).color(col));
        }
    });
    ui.horizontal_wrapped(|ui| {
        for &(label, v) in chips {
            if crate::ui::widgets::chip(ui, label, *secs == v).on_hover_text(if min == 0 { "Start this long from now" } else { "Use this length" }).clicked() {
                *secs = v;
                changed = true;
                ui.data_mut(|d| d.remove::<String>(egui::Id::new(("off_len_text", id))));
            }
        }
    });
    changed
}

pub fn drag_min(ui: &mut Ui, v: &mut u32, range: std::ops::RangeInclusive<u32>, suffix: &str) -> bool {
    ui.add(egui::DragValue::new(v).range(range).suffix(suffix)).on_hover_text("Drag, or double-click to type a number").changed()
}
