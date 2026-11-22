use crate::engine::Sched;
use crate::ui::parts::schedule_cards;
use crate::ui::{theme, widgets, App, Kind};

const CARD_H: f32 = 72.0;
use eframe::egui::{RichText, Ui};

/// Quick "start in" offsets in minutes from now.
const START_IN: [(&str, u32); 9] = [("5 min", 5), ("10 min", 10), ("15 min", 15), ("30 min", 30), ("45 min", 45), ("1 h", 60), ("2 h", 120), ("4 h", 240), ("8 h", 480)];
/// Quick screen-off lengths in minutes.
const OFF_FOR: [(&str, u32); 10] = [("5 min", 5), ("10 min", 10), ("15 min", 15), ("20 min", 20), ("30 min", 30), ("45 min", 45), ("1 h", 60), ("1 h 30 min", 90), ("2 h", 120), ("4 h", 240)];

pub struct Form {
    pub h: u32,
    pub m: u32,
    pub mins: u32,
    pub daily: bool,
    pub picked: Vec<String>,
}

impl Default for Form {
    fn default() -> Self {
        Form { h: 23, m: 0, mins: 30, daily: true, picked: Vec::new() }
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Schedule");
    let mut cancel: Option<u32> = None;
    let scheds = app.engine.settings().scheds.clone();
    let next = app.view().next_sched;
    widgets::card(ui, "Planned screen-offs", |ui| {
        if scheds.is_empty() {
            crate::ui::parts::empty_state::empty_page(ui, crate::ui::parts::empty_state::EmptyPage::Schedule);
        }
        for s in &scheds {
            ui.horizontal(|ui| {
                let card_w = (ui.available_width() - 180.0).max(120.0);
                ui.allocate_ui(eframe::egui::vec2(card_w, CARD_H), |ui| {
                    // Display-only card: the row's Cancel button removes the schedule, so the
                    // card's own toggle is reset every frame and never changes state.
                    let mut on = true;
                    let when = format!("{:02}:{:02}", s.h, s.m);
                    let detail = format!("{} min · {} screen(s)", s.mins, s.targets.len());
                    if s.daily {
                        schedule_cards::daily_card(ui, &when, &format!("Every day · {detail}"), &mut on);
                    } else {
                        schedule_cards::once_card(ui, &format!("{when} · {detail}"), &mut on);
                    }
                });
                if next.map(|n| n.0) == Some(s.id) {
                    widgets::status_pill(ui, "Next", widgets::PillKind::Info);
                }
                if widgets::ghost_button(ui, "Cancel").on_hover_text("Remove this planned screen-off").clicked() {
                    cancel = Some(s.id);
                }
            });
            ui.add_space(6.0);
        }
    });
    if let Some(id) = cancel {
        let acts = app.engine.cancel_sched(id);
        app.dispatch(acts);
        app.mark_dirty();
    }
    ui.add_space(8.0);
    let mons = app.monitors.clone();
    let mut add = false;
    widgets::card(ui, "Add a schedule", |ui| {
        let f = &mut app.sched_form;
        ui.horizontal(|ui| {
            ui.label("At");
            super::drag_min(ui, &mut f.h, 0..=23, " h");
            super::drag_min(ui, &mut f.m, 0..=59, " m");
            ui.label("for");
            super::drag_min(ui, &mut f.mins, 1..=600, " min");
        });
        let now = crate::sys::local_now();
        let now_min = now.hour as u32 * 60 + now.minute as u32;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Start in").color(theme::pal().text_dim));
            for (label, d) in START_IN {
                if widgets::chip(ui, label, false).on_hover_text("Set the start time to this long from now (runs once)").clicked() {
                    let t = (now_min + d) % 1440;
                    f.h = t / 60;
                    f.m = t % 60;
                    f.daily = false;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("For").color(theme::pal().text_dim));
            for (label, m) in OFF_FOR {
                if widgets::chip(ui, label, f.mins == m).on_hover_text("Keep the screens off this long").clicked() {
                    f.mins = m;
                }
            }
        });
        let wait = (f.h * 60 + f.m + 1440 - now_min) % 1440;
        ui.label(
            RichText::new(format!(
                "Turns off at {:02}:{:02} (in {}) for {}",
                f.h,
                f.m,
                crate::engine::fmt_duration(wait * 60),
                crate::engine::fmt_duration(f.mins * 60)
            ))
            .color(theme::pal().text_dim),
        );
        crate::ui::widgets::checkbox(ui, &mut f.daily, "Repeat every day").on_hover_text("Run this every day at the same time");
        for (i, m) in mons.iter().enumerate() {
            let mut on = f.picked.contains(&m.name);
            if crate::ui::widgets::checkbox(ui, &mut on, format!("Screen {}", i + 1)).on_hover_text("Turn this screen off for the schedule").changed() {
                if on {
                    f.picked.push(m.name.clone());
                } else {
                    f.picked.retain(|x| x != &m.name);
                }
            }
        }
        if widgets::primary_button(ui, "Add").on_hover_text("Save this schedule").clicked() {
            add = true;
        }
    });
    if add {
        let f = &app.sched_form;
        let targets = if f.picked.is_empty() { app.selected_or_primary() } else { f.picked.clone() };
        let s = Sched { id: 0, h: f.h as u8, m: f.m as u8, mins: f.mins, targets, daily: f.daily };
        match app.engine.add_sched(s) {
            Ok(_) => {
                app.toast(Kind::Ok, "Schedule added");
                app.mark_dirty();
            }
            Err(e) => app.toast(Kind::Err, e),
        }
    }
}
