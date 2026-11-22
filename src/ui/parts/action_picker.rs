//! Box 8: break-action picker (cards + chips) with monitor selector and duration.
//! State lives in one process-wide cell so the engine side (box 9) and
//! persistence (box 10) can read/write it via `current()` / `set()`.
#![allow(dead_code)]

use crate::break_action::{actions_from_text, actions_to_text, ActionKind, BreakAction, MAX_ACTION_SECS};
use crate::sys::MonitorInfo;
use crate::ui::icons::{self, Icon};
use crate::ui::theme::{self, accent, accent_hi, mix, pal, rad};
use crate::ui::widgets;
use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Stroke, StrokeKind, Ui};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerState {
    /// Chosen break-start actions (each with its own duration; 0 = whole break).
    pub actions: Vec<BreakAction>,
    /// Monitor names the monitor actions apply to; empty = by action kind (main / secondary / all).
    pub monitors: Vec<String>,
}

impl Default for PickerState {
    fn default() -> Self {
        PickerState { actions: vec![BreakAction::new(ActionKind::OverlayOnly, 0)], monitors: Vec::new() }
    }
}

impl PickerState {
    /// `actions|mon1,mon2` - one settings value.
    pub fn to_text(&self) -> String {
        format!("{}|{}", actions_to_text(&self.actions), self.monitors.join(","))
    }
    pub fn from_text(s: &str) -> PickerState {
        let (a, m) = s.split_once('|').unwrap_or((s, ""));
        PickerState { actions: actions_from_text(a), monitors: m.split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().to_string()).collect() }
    }
}

fn cell() -> &'static Mutex<PickerState> {
    static C: OnceLock<Mutex<PickerState>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(PickerState::default()))
}

pub fn current() -> PickerState {
    cell().lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn set(s: PickerState) {
    if let Ok(mut g) = cell().lock() {
        *g = s;
    }
}

fn icon_for(k: &ActionKind) -> Icon {
    match k {
        ActionKind::None => Icon::Clear,
        ActionKind::OverlayOnly => Icon::Cup,
        ActionKind::SleepMainMonitor | ActionKind::SleepSecondaryMonitors | ActionKind::SleepAllMonitors => Icon::Monitor,
        ActionKind::Dim => Icon::Moon,
        ActionKind::LockWorkstation => Icon::Power,
        ActionKind::MuteAudio => Icon::Bell,
        ActionKind::PauseMedia => Icon::Pause,
        ActionKind::MinimizeAllWindows => Icon::Collapse,
        ActionKind::CustomCommand(_) => Icon::Gear,
    }
}

fn hint(k: &ActionKind) -> &'static str {
    match k {
        ActionKind::None => "Nothing happens",
        ActionKind::OverlayOnly => "Break screen only",
        ActionKind::SleepMainMonitor => "Primary screen off",
        ActionKind::SleepSecondaryMonitors => "Other screens off",
        ActionKind::SleepAllMonitors => "Every screen off",
        ActionKind::Dim => "Screens dim",
        ActionKind::LockWorkstation => "Win+L lock",
        ActionKind::MuteAudio => "Silence speakers",
        ActionKind::PauseMedia => "Stop video/music",
        ActionKind::MinimizeAllWindows => "Show desktop",
        ActionKind::CustomCommand(_) => "Run your command",
    }
}

fn same_kind(a: &ActionKind, b: &ActionKind) -> bool {
    a.key() == b.key()
}

fn uses_monitors(k: &ActionKind) -> bool {
    matches!(k, ActionKind::SleepMainMonitor | ActionKind::SleepSecondaryMonitors | ActionKind::SleepAllMonitors | ActionKind::Dim | ActionKind::OverlayOnly)
}

/// Pure toggle logic (tested): None is exclusive; picking anything clears None.
pub fn toggle_kind(list: &mut Vec<BreakAction>, k: &ActionKind) {
    if let Some(i) = list.iter().position(|a| same_kind(&a.kind, k)) {
        list.remove(i);
    } else if matches!(k, ActionKind::None) {
        list.clear();
        list.push(BreakAction::new(ActionKind::None, 0));
    } else {
        list.retain(|a| !matches!(a.kind, ActionKind::None));
        list.push(BreakAction::new(k.clone(), 0));
    }
    if list.is_empty() {
        list.push(BreakAction::new(ActionKind::None, 0));
    }
}

/// One selectable action card. Returns the click response.
fn action_card(ui: &mut Ui, k: &ActionKind, selected: bool) -> egui::Response {
    let size = vec2(206.0, 74.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), theme::ANIM);
    let base = if selected { mix(pal().surface, accent(), 0.22) } else { pal().surface_hi };
    let fill = mix(base, accent(), t * 0.10);
    let stroke = if selected { Stroke::new(2.0_f32, accent()) } else { Stroke::new(1.0_f32, pal().border_strong) };
    let p = ui.painter();
    p.rect_filled(rect, rad(14), fill);
    p.rect_stroke(rect, rad(14), stroke, StrokeKind::Inside);
    let ic = pos2(rect.left() + 22.0, rect.top() + 24.0);
    p.circle_filled(ic, 15.0, mix(pal().surface, accent(), if selected { 0.45 } else { 0.15 }));
    icons::paint(p, icon_for(k), ic, 16.0, if selected { accent_hi() } else { pal().text });
    p.text(pos2(rect.left() + 44.0, rect.top() + 24.0), egui::Align2::LEFT_CENTER, k.label(), egui::FontId::proportional(13.0), pal().text);
    p.text(pos2(rect.left() + 12.0, rect.bottom() - 16.0), egui::Align2::LEFT_CENTER, hint(k), egui::FontId::proportional(11.5), pal().text_dim);
    if selected {
        let c = pos2(rect.right() - 14.0, rect.top() + 14.0);
        p.circle_filled(c, 8.0, accent());
        p.add(egui::Shape::line(vec![c + vec2(-3.5, 0.0), c + vec2(-1.0, 3.0), c + vec2(4.0, -3.5)], Stroke::new(2.0_f32, Color32::WHITE)));
    }
    resp.on_hover_text(format!("{}: {}", k.label(), hint(k)))
}

fn dur_label(secs: u32) -> String {
    if secs == 0 {
        "whole break".into()
    } else if secs % 60 == 0 {
        format!("{} min", secs / 60)
    } else {
        format!("{secs} s")
    }
}

/// Draw the picker. Returns true when the state changed.
pub fn picker(ui: &mut Ui, st: &mut PickerState, monitors: &[MonitorInfo]) -> bool {
    let mut changed = false;
    ui.label(RichText::new("When a break starts").color(pal().text_dim));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(theme::SP_S, theme::SP_S);
        for k in ActionKind::all() {
            let sel = st.actions.iter().any(|a| same_kind(&a.kind, &k));
            if action_card(ui, &k, sel).clicked() {
                toggle_kind(&mut st.actions, &k);
                changed = true;
            }
        }
    });
    ui.add_space(theme::SP_S);
    if st.actions.iter().any(|a| uses_monitors(&a.kind)) {
        ui.label(RichText::new("Monitors (none picked = by action: main / secondary / all)").color(pal().text_dim));
        ui.horizontal_wrapped(|ui| {
            if monitors.is_empty() {
                ui.label(RichText::new("Detecting monitors...").color(pal().text_dim));
            }
            for (i, m) in monitors.iter().enumerate() {
                let sel = st.monitors.iter().any(|n| n == &m.name);
                let label = format!("{} {}{}  {}x{}", "Screen", i + 1, if m.primary { " (main)" } else { "" }, m.w, m.h);
                if widgets::chip(ui, &label, sel).on_hover_text(&m.name).clicked() {
                    if sel {
                        st.monitors.retain(|n| n != &m.name);
                    } else {
                        st.monitors.push(m.name.clone());
                    }
                    changed = true;
                }
            }
        });
        ui.add_space(theme::SP_XS);
    }
    for a in st.actions.iter_mut().filter(|a| !matches!(a.kind, ActionKind::None)) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{} lasts", a.kind.label())).strong());
            for secs in [0u32, 60, 120, 300, 600] {
                if widgets::chip(ui, &dur_label(secs), a.duration_secs == secs).clicked() {
                    a.duration_secs = secs;
                    changed = true;
                }
            }
            let mut m = a.duration_secs / 60;
            let mut s = a.duration_secs % 60;
            let r1 = ui.add(egui::DragValue::new(&mut m).range(0..=MAX_ACTION_SECS / 60).suffix(" min"));
            let r2 = ui.add(egui::DragValue::new(&mut s).range(0..=59).suffix(" s"));
            if r1.changed() || r2.changed() {
                a.duration_secs = (m * 60 + s).min(MAX_ACTION_SECS);
                changed = true;
            }
        });
        if let ActionKind::CustomCommand(cmd) = &mut a.kind {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Command").color(pal().text_dim));
                if ui.add(egui::TextEdit::singleline(cmd).hint_text("e.g. nircmd.exe monitor off").desired_width(320.0)).changed() {
                    changed = true;
                }
            });
        }
    }
    changed
}

/// Load the saved picker (settings `break_actions` / `break_monitors`) into the cell once per process.
pub fn seed_once(actions: &str, monitors: &str) {
    static SEEDED: OnceLock<()> = OnceLock::new();
    SEEDED.get_or_init(|| {
        if !actions.trim().is_empty() {
            set(PickerState::from_text(&format!("{actions}|{monitors}")));
        }
    });
}

/// Dashboard card wrapper using the shared state cell. Returns true when the user changed it.
pub fn card(ui: &mut Ui, monitors: &[MonitorInfo]) -> bool {
    let mut st = current();
    let mut changed = false;
    widgets::card(ui, "Break action", |ui| {
        if picker(ui, &mut st, monitors) {
            set(st.clone());
            changed = true;
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_none_is_exclusive_and_text_roundtrip() {
        let mut l = vec![BreakAction::new(ActionKind::None, 0)];
        toggle_kind(&mut l, &ActionKind::SleepAllMonitors);
        toggle_kind(&mut l, &ActionKind::MuteAudio);
        assert_eq!(l.len(), 2);
        assert!(!l.iter().any(|a| a.kind == ActionKind::None));
        toggle_kind(&mut l, &ActionKind::None);
        assert_eq!(l, vec![BreakAction::new(ActionKind::None, 0)]);
        let st = PickerState { actions: vec![BreakAction::new(ActionKind::SleepSecondaryMonitors, 300)], monitors: vec!["\\\\.\\DISPLAY2".into()] };
        assert_eq!(PickerState::from_text(&st.to_text()), st);
    }
}
