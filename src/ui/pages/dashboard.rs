use crate::engine::Phase;
use crate::ui::icons::Icon;
use crate::ui::widgets::{self, PillKind, TileState};
use crate::ui::{theme, App};
use eframe::egui::{RichText, Ui};

fn screens_card(app: &mut App, ui: &mut Ui, view: &crate::engine::EngineView) {
    widgets::card(ui, "Quick screen-off", |ui| {
        ui.label(RichText::new("Start in (from now)").color(theme::pal().text_dim));
        app.off_delay_edit(ui, "dash-delay");
        ui.add_space(theme::SP_XS);
        ui.label(RichText::new("Turn off for (one tap starts it)").color(theme::pal().text_dim));
        ui.horizontal_wrapped(|ui| {
            for m in [1u32, 5, 10, 20, 30, 60] {
                if widgets::chip(ui, &format!("{m} min"), false).on_hover_text("Turn the selected screens off for this long, after the Start in time").clicked() {
                    let t = app.selected_or_primary();
                    app.off_after_delay(t, m * 60);
                }
            }
        });
        ui.label(RichText::new("Exact length").color(theme::pal().text_dim));
        app.off_len_edit(ui, "dash");
        if widgets::primary_button(ui, &app.off_chosen_label()).on_hover_text("Turn the selected screens off after the Start in time, for exactly the chosen length").clicked() {
            let t = app.selected_or_primary();
            app.off_chosen(t);
        }
        app.delayed_off_rows(ui);
        if !app.monitors_loaded {
            ui.label(RichText::new("Detecting monitors...").color(theme::pal().text_dim));
        } else if app.monitors.is_empty() {
            crate::ui::parts::empty_state::empty_page(ui, crate::ui::parts::empty_state::EmptyPage::Monitors);
        }
        let mons = app.monitors.clone();
        ui.horizontal_wrapped(|ui| {
            for m in &mons {
                let off = view.active_off.iter().any(|(t, _)| t.contains(&m.name));
                let sel = app.is_selected(&m.name);
                let st = if off {
                    TileState::Off
                } else if sel {
                    TileState::Selected
                } else {
                    TileState::Idle
                };
                if widgets::monitor_tile(ui, m, sel, st).on_hover_text("Click to pick this screen for breaks and screen-off").clicked() {
                    app.toggle_selected(&m.name);
                }
            }
        });
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        for (targets, end) in &view.active_off {
            ui.horizontal(|ui| {
                let left = super::fmt_mmss((end - now).max(0) as u32);
                widgets::status_pill(ui, &format!("{} screen(s) off, back in {left}", targets.len()), PillKind::Warn);
                if widgets::ghost_button(ui, "End now").on_hover_text("Bring the screens back immediately").clicked() {
                    app.end_off(targets.clone());
                }
                if widgets::ghost_button(ui, "Extend +5 min").on_hover_text("Keep the screens off 5 minutes longer").clicked() {
                    app.extend_off(targets.clone(), 300);
                }
            });
        }
    });
}

fn onboarding_card(app: &mut App, ui: &mut Ui) {
    widgets::hero_card(ui, theme::accent(), |ui| {
        ui.label(RichText::new("Welcome to BreakGuard").heading());
        ui.label(RichText::new("Pick how often you want to rest your eyes. You can change it any time on the Breaks page.").color(theme::pal().text_dim));
        ui.add_space(theme::SP_S);
        let mut chosen: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (i, (name, w, b)) in crate::logic::PRESETS.iter().enumerate() {
                let label = format!("{name}: {w} min work, {} break", super::fmt_dur(*b));
                if widgets::ghost_button(ui, &label).on_hover_text("Start the timer with this rhythm").clicked() {
                    chosen = Some(i);
                }
            }
        });
        if let Some(i) = chosen {
            app.finish_onboarding(Some(i));
        }
        if widgets::ghost_button(ui, "Keep my numbers and start").clicked() {
            app.finish_onboarding(None);
        }
    });
    ui.add_space(theme::SP_S);
}

fn timer_hero(app: &mut App, ui: &mut Ui, view: &crate::engine::EngineView) {
    let s = app.engine.settings().clone();
    let (frac, big, small, color, kind, state) = match view.phase {
        Phase::Off => (0.0, "--:--".to_string(), "timer off", theme::pal().text_dim, PillKind::Neutral, "Timer off"),
        Phase::Working { left } => (
            left as f32 / (s.work_min.max(1) * 60) as f32,
            super::fmt_mmss(left),
            if view.paused { "paused" } else { "until break" },
            if view.paused { theme::pal().text_dim } else { theme::accent() },
            if view.paused { PillKind::Neutral } else { PillKind::Info },
            if view.paused { "Paused" } else { "Working" },
        ),
        Phase::Warning { left } => (left as f32 / s.warn_secs.max(1) as f32, super::fmt_mmss(left), "break soon", theme::WARN, PillKind::Warn, "Break soon"),
        Phase::Break { left } => (left as f32 / view.break_total.max(1) as f32, super::fmt_mmss(left), "rest your eyes", theme::OK, PillKind::Ok, "On break"),
    };
    widgets::hero_card(ui, color, |ui| {
        let side = crate::ui::parts::timer_setup::hero_controls_fit(ui.available_width());
        ui.horizontal(|ui| {
            let ring_h = widgets::countdown_ring(ui, frac.clamp(0.0, 1.0), &big, small, color).rect.height();
            ui.add_space(theme::SP_M);
            let rest_w = ui.available_width();
            ui.vertical(|ui| {
                if side {
                    ui.set_max_width((rest_w * 0.42).max(260.0));
                }
                ui.set_min_height(ring_h);
                ui.horizontal(|ui| {
                    widgets::status_pill(ui, state, kind);
                    match view.hold {
                        crate::engine::Hold::Idle => {
                            widgets::status_pill(ui, "Away: timer frozen", PillKind::Neutral);
                        }
                        crate::engine::Hold::Busy => {
                            widgets::status_pill(ui, "Fullscreen app: timer frozen", PillKind::Neutral);
                        }
                        crate::engine::Hold::OffHours => {
                            widgets::status_pill(ui, "Outside work hours", PillKind::Neutral);
                        }
                        crate::engine::Hold::None => {}
                    }
                });
                ui.label(RichText::new(format!("Work {}, then a {} break", super::fmt_dur(s.work_min * 60), super::fmt_dur(s.break_secs))).size(16.0));
                work_editor(app, ui);
                break_editor(app, ui);
                if let Some((_, at)) = view.next_sched {
                    ui.label(RichText::new(format!("Next scheduled screen-off at {}", super::fmt_hm(at))).color(theme::pal().text_dim));
                }
                ui.add_space(theme::SP_XS);
                actions(app, ui, view);
            });
            if side {
                ui.add_space(theme::SP_M);
                ui.separator();
                ui.add_space(theme::SP_M);
                if crate::ui::parts::timer_setup::show(app, ui) {
                    app.mark_dirty();
                }
            }
        });
        if !side {
            ui.add_space(theme::SP_S);
            if crate::ui::parts::timer_setup::show(app, ui) {
                app.mark_dirty();
            }
        }
    });
}

/// Work length before each break: any whole minute from 1 up to 24 h.
/// Drag the value, double-click it to type ("60", "1h30", "2:15"), or tap a chip.
fn work_editor(app: &mut App, ui: &mut Ui) {
    let max = crate::engine::MAX_WORK_MIN;
    let mut v = app.engine.settings().work_min;
    let before = v;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Work for").color(theme::pal().text_dim));
        ui.add(
            eframe::egui::DragValue::new(&mut v)
                .range(1..=max)
                .speed(0.5)
                .custom_formatter(|n, _| super::fmt_dur((n.round() as u32).max(1) * 60))
                .custom_parser(|t| crate::engine::parse_duration(t).map(|secs| (secs as f64 / 60.0).round().max(1.0))),
        )
        .on_hover_text("Drag, or double-click and type any length: 60, 1h30, 2:15 (1 min to 24 h)");
        for m in [20u32, 25, 30, 45, 60, 90, 120] {
            if widgets::chip(ui, &super::fmt_dur(m * 60), v == m).on_hover_text("Work this long before each break").clicked() {
                v = m;
            }
        }
    });
    let v = v.clamp(1, max);
    if v != before {
        app.engine.settings_mut().work_min = v;
        app.mark_dirty();
    }
}

/// Break length, right beside the timer: any whole second from 5 s up to the max.
/// Drag the value, double-click to type ("20s", "90s", "2m", "1m30s"), or tap a chip.
fn break_editor(app: &mut App, ui: &mut Ui) {
    let max = crate::engine::BREAK_MAX_SECS;
    let mut v = app.engine.settings().break_secs;
    let before = v;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Break for").color(theme::pal().text_dim));
        ui.add(
            eframe::egui::DragValue::new(&mut v)
                .range(5..=max)
                .speed(1.0)
                .custom_formatter(|n, _| super::fmt_dur((n.round() as u32).max(5)))
                .custom_parser(|t| crate::engine::parse_duration(t).map(|secs| (secs as f64).max(5.0))),
        )
        .on_hover_text("Drag, or double-click and type any length: 20s, 90s, 2m, 1m30s (5 s to 4 h)");
        for b in [20u32, 30, 60, 120, 300, 600] {
            if widgets::chip(ui, &super::fmt_dur(b), v == b).on_hover_text("Rest this long each break").clicked() {
                v = b;
            }
        }
    });
    let v = v.clamp(5, max);
    if v != before {
        let st = app.engine.settings_mut();
        st.break_secs = v;
        st.clamp();
        app.engine.apply_settings_changed();
        app.mark_dirty();
    }
}

/// One-tap actions: Break now, Pause, Snooze 5/10/15, Skip, Extend.
fn actions(app: &mut App, ui: &mut Ui, view: &crate::engine::EngineView) {
    let in_break = matches!(view.phase, Phase::Break { .. });
    ui.horizontal_wrapped(|ui| {
        if matches!(view.phase, Phase::Off) {
            if widgets::primary_button(ui, "Start timer").on_hover_text("Start counting down to the next break").clicked() {
                app.engine.start_break_timer();
            }
            if widgets::ghost_button(ui, "Break now").on_hover_text("Start a break right now (Ctrl+B)").clicked() {
                app.engine.start_break_now();
            }
            return;
        }
        if !in_break && widgets::primary_button(ui, "Break now").on_hover_text("Start a break right now (Ctrl+B)").clicked() {
            app.engine.start_break_now();
        }
        let paused = view.paused;
        if widgets::ghost_button(ui, if paused { "Resume" } else { "Pause" }).on_hover_text("Freeze or continue the countdown (Ctrl+P)").clicked() {
            app.toggle_pause();
        }
        if in_break {
            if app.engine.settings().allow_skip {
                if widgets::primary_button(ui, "Skip break").on_hover_text("End this break early").clicked() {
                    app.engine.skip_break();
                }
            } else {
                widgets::status_pill(ui, "Strict mode: no skipping", PillKind::Neutral);
            }
            if widgets::ghost_button(ui, "+1 min").on_hover_text("Rest one more minute").clicked() {
                app.extend_break(60);
            }
        }
    });
    if !matches!(view.phase, Phase::Off) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Snooze").color(theme::pal().text_dim));
            for m in [5u32, 10, 15] {
                if widgets::chip(ui, &format!("{m} min"), false).on_hover_text("Push the next break back").clicked() {
                    app.engine.snooze(m);
                }
            }
            if widgets::chip(ui, &format!("{} min (yours)", app.engine.settings().snooze_mins), false).on_hover_text("Your own snooze length from the Breaks page").clicked() {
                app.engine.snooze_default();
            }
            if widgets::ghost_button(ui, "Stop timer").on_hover_text("Turn the break timer off").clicked() {
                app.engine.stop_break_timer();
            }
        });
    }
}

fn stats_row(app: &mut App, ui: &mut Ui) {
    let today = app.engine.today();
    let d = app.engine.stats().day(today);
    let goal = app.engine.settings().daily_goal;
    let streak = app.engine.stats().streak(today, 1);
    ui.horizontal_wrapped(|ui| {
        widgets::stat_tile(ui, Icon::Cup, &d.taken.to_string(), "Breaks today", theme::accent());
        widgets::stat_tile(ui, Icon::Target, &format!("{}/{}", d.taken.min(999), goal), "Daily goal", theme::OK);
        widgets::stat_tile(ui, Icon::Flame, &format!("{streak} d"), "Streak", theme::WARN);
        widgets::stat_tile(ui, Icon::Skip, &(d.skipped + d.snoozed).to_string(), "Skipped / snoozed", theme::DANGER);
    });
    ui.add_space(theme::SP_XS);
    widgets::progress_bar(ui, crate::logic::goal_fraction(d.taken, goal), theme::accent(), 10.0);
    ui.label(RichText::new(crate::logic::greeting(d.taken, goal, streak)).color(theme::pal().text_dim));
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Dashboard");
    let view = app.view();
    if !app.engine.settings().onboarded {
        onboarding_card(app, ui);
    }
    timer_hero(app, ui, &view);
    ui.add_space(theme::SP_S);
    super::break_duration::card(app, ui);
    ui.add_space(theme::SP_S);
    stats_row(app, ui);
    ui.add_space(theme::SP_S);
    if app.engine.settings().tips {
        let tip = crate::tips::pick(&app.engine.settings().custom_tips, view.cycle, view.break_is_long);
        widgets::card(ui, "Next tip", |ui| {
            ui.label(RichText::new(tip).size(15.0));
        });
        ui.add_space(theme::SP_S);
    }
    screens_card(app, ui, &view);
    ui.add_space(theme::SP_S);
    let mons = app.monitors.clone();
    {
        let s = app.engine.settings();
        crate::ui::parts::action_picker::seed_once(&s.break_actions, &s.break_monitors);
    }
    if crate::ui::parts::action_picker::card(ui, &mons) {
        // Persist the picker in settings.ini (break_exec reads `break_actions=` at break start).
        let st = crate::ui::parts::action_picker::current();
        let s = app.engine.settings_mut();
        s.break_actions = crate::break_action::actions_to_text(&st.actions);
        s.break_monitors = st.monitors.join(",");
        app.mark_dirty();
    }
    ui.add_space(theme::SP_S);
    widgets::card(ui, "Sleep", |ui| {
        ui.horizontal(|ui| {
            let mut v = app.engine.settings().sleep_min;
            if super::drag_min(ui, &mut v, 1..=720, " min") {
                app.engine.settings_mut().sleep_min = v;
                app.mark_dirty();
            }
            ui.add_enabled_ui(!app.sleeping, |ui| {
                if widgets::danger_button(ui, "Put PC to sleep").on_hover_text("Sleep now and wake after this many minutes").clicked() {
                    app.confirm_sleep = true;
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn dashboard_break_input_parses_seconds() {
        use crate::engine::parse_duration;
        assert_eq!(parse_duration("45s"), Some(45));
        assert_eq!(parse_duration("90s"), Some(90));
        assert_eq!(parse_duration("2m"), Some(120));
    }
}
