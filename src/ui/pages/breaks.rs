use crate::engine::{BreakStyle, BreakTarget, OffMode, Phase};
use crate::logic::{preset_index, PRESETS};
use crate::ui::parts::break_ring;
use crate::ui::{theme, widgets, App};
use eframe::egui::{self, vec2, Align2, RichText, Sense, Stroke, StrokeKind, Ui};

/// One selectable preset card (name, rhythm line); returns true when clicked.
fn preset_card(ui: &mut Ui, name: &str, line: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(150.0, 78.0), Sense::click());
    let t = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), theme::ANIM);
    let fill = theme::mix(if selected { theme::mix(theme::SURFACE, theme::accent(), 0.16) } else { theme::SURFACE }, theme::SURFACE_HI, t);
    let p = ui.painter();
    p.rect_filled(rect, theme::rad(theme::R_M), fill);
    p.rect_stroke(rect, theme::rad(theme::R_M), Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { theme::accent() } else { theme::BORDER_STRONG }), StrokeKind::Inside);
    p.text(rect.left_top() + vec2(14.0, 24.0), Align2::LEFT_CENTER, name, theme::num(16.0), theme::pal().text);
    p.text(rect.left_top() + vec2(14.0, 50.0), Align2::LEFT_CENTER, line, egui::FontId::proportional(12.5), theme::pal().text_dim);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, name));
    resp.on_hover_text(format!("Use {name}: {line}")).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// Running Claude Code agents: pause during breaks (all or chosen), Resume now, per-agent Break / Continue.
fn agents_card(app: &mut App, ui: &mut Ui, s: &mut crate::engine::Settings, changed: &mut bool) {
    app.refresh_agents(false);
    static DEMO: std::sync::Once = std::sync::Once::new();
    if crate::sys::is_mock() && std::env::var_os("BREAKGUARD_PAUSE_DEMO").is_some() && !app.agents.is_empty() {
        DEMO.call_once(|| {
            let first = app.agents[0].clone();
            app.pauser.pause(std::slice::from_ref(&first));
        });
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
    widgets::card(ui, "Claude Code agents", |ui| {
        *changed |= widgets::setting_toggle(ui, &mut s.pause_agents, "Pause agents during breaks", "Running agents are suspended (never killed) while a break runs and resume when it ends.");
        if app.pauser.any_paused() {
            ui.horizontal(|ui| {
                widgets::status_pill(ui, "Agents paused", widgets::PillKind::Warn);
                if widgets::primary_button(ui, "Resume now").on_hover_text("Wake every paused agent right away").clicked() {
                    app.resume_agents_now();
                }
            });
        }
        let list = app.agents.clone();
        if list.is_empty() {
            ui.label(RichText::new("No running agents found.").color(theme::pal().text_dim));
        }
        for a in &list {
            ui.horizontal_wrapped(|ui| {
                let paused = app.pauser.is_paused(&a.session_id);
                let mut chosen = s.pause_sel.is_empty() || s.pause_sel.contains(&a.session_id);
                let check = widgets::checkbox(ui, &mut chosen, "").on_hover_text("Include this agent when a break pauses agents");
                check.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Checkbox, true, &a.name));
                if check.changed() {
                    if s.pause_sel.is_empty() {
                        s.pause_sel = list.iter().map(|x| x.session_id.clone()).collect();
                    }
                    if chosen {
                        if !s.pause_sel.contains(&a.session_id) {
                            s.pause_sel.push(a.session_id.clone());
                        }
                    } else {
                        s.pause_sel.retain(|x| x != &a.session_id);
                    }
                    if s.pause_sel.len() == list.len() {
                        s.pause_sel.clear();
                    }
                    *changed = true;
                }
                ui.label(RichText::new(&a.name).strong());
                ui.label(RichText::new(format!("pid {}", a.pid)).size(12.0).color(theme::pal().text_dim));
                if paused {
                    widgets::status_pill(ui, "Paused", widgets::PillKind::Warn);
                }
                let label = if paused { "Continue" } else { "Break" };
                if widgets::ghost_button(ui, label).on_hover_text(if paused { "Resume just this agent" } else { "Pause just this agent now" }).clicked() {
                    app.toggle_agent(a);
                }
            });
        }
    });
}

/// Live break status: animated countdown ring plus one status card per monitor.
fn status_card(app: &App, ui: &mut Ui) {
    let view = app.view();
    let (frac, big, state, ok) = match view.phase {
        Phase::Off => (0.0, "--:--".to_string(), "Timer off", false),
        Phase::Working { left } => (
            left as f32 / (app.engine.settings().work_min.max(1) * 60) as f32,
            super::fmt_mmss(left),
            "Working",
            true,
        ),
        Phase::Warning { left } => (
            left as f32 / app.engine.settings().warn_secs.max(1) as f32,
            super::fmt_mmss(left),
            "Break soon",
            true,
        ),
        Phase::Break { left } => (
            left as f32 / view.break_total.max(1) as f32,
            super::fmt_mmss(left),
            "On break",
            false,
        ),
    };
    if !matches!(view.phase, Phase::Off) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
    widgets::card(ui, "Break status", |ui| {
        ui.horizontal(|ui| {
            break_ring::countdown_ring(ui, 120.0, frac, &big);
            ui.add_space(theme::SP_M);
            ui.vertical(|ui| {
                ui.label(RichText::new(state).strong().color(theme::pal().text));
                if app.monitors.is_empty() {
                    ui.label(RichText::new("No screens detected").size(12.0).color(theme::pal().text_dim));
                }
            });
        });
        if !app.monitors.is_empty() {
            ui.add_space(theme::SP_S);
            ui.columns(app.monitors.len().min(3), |cols| {
                for (i, m) in app.monitors.iter().enumerate().take(3) {
                    let col = &mut cols[i];
                    if ok {
                        break_ring::monitor_card(col, &m.name, "Ready", true);
                    } else {
                        break_ring::monitor_card(col, &m.name, "Screen off", false);
                    }
                }
            });
        }
    });
    ui.add_space(theme::SP_S);
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Breaks");
    status_card(app, ui);
    let mut s = app.engine.settings().clone();
    let mut changed = false;

    widgets::card(ui, "Take a break now", |ui| {
        ui.label(RichText::new("Pick the screens, when it starts (from now) and how long they stay off.").size(12.0).color(theme::pal().text_dim));
        ui.add_space(theme::SP_XS);
        let n = app.monitors.len();
        let mut t = app.quick_target;
        ui.horizontal_wrapped(|ui| {
            for (target, label) in [(BreakTarget::Mon1, "Screen 1"), (BreakTarget::Mon2, "Screen 2"), (BreakTarget::Both, "Both screens")] {
                let usable = match target {
                    BreakTarget::Mon1 => n >= 1,
                    BreakTarget::Mon2 => n >= 2,
                    BreakTarget::Both => n >= 1,
                };
                ui.add_enabled_ui(usable, |ui| {
                    if widgets::chip(ui, label, t == target).on_hover_text("Which screens turn off").clicked() {
                        t = target;
                    }
                });
            }
        });
        app.quick_target = t;
        ui.label(RichText::new("Length").color(theme::pal().text_dim));
        app.off_len_edit(ui, "breaks");
        ui.horizontal_wrapped(|ui| {
            for p in s.presets.clone() {
                if widgets::chip(ui, &format!("{} {} min", p.name, p.mins), false).on_hover_text("Fill in this preset").clicked() {
                    app.engine.settings_mut().off_secs = p.mins.saturating_mul(60).clamp(1, crate::engine::MAX_OFF_SECS);
                    app.mark_dirty();
                    app.quick_target = p.target;
                    app.quick_pause = p.pause_agents;
                }
            }
        });
        let mut qp = app.quick_pause;
        if widgets::chip(ui, "Pause agents", qp).on_hover_text("Suspend running Claude Code agents until this break ends").clicked() {
            qp = !qp;
        }
        app.quick_pause = qp;
        ui.add_space(theme::SP_XS);
        ui.label(RichText::new("Start in (from now)").color(theme::pal().text_dim));
        app.off_delay_edit(ui, "breaks-delay");
        ui.add_space(theme::SP_XS);
        let devs = app.target_devs(app.quick_target);
        ui.add_enabled_ui(!devs.is_empty(), |ui| {
            let secs = app.off_secs();
            let delay = app.off_delay;
            let label = if delay == 0 { format!("Start {} break", crate::engine::fmt_duration(secs)) } else { format!("Start {} break in {}", crate::engine::fmt_duration(secs), crate::engine::fmt_duration(delay)) };
            if widgets::primary_button(ui, &label).on_hover_text("Turn the chosen screens off after the Start in time, for the chosen length").clicked() {
                app.off_after_delay(devs, secs);
                if app.quick_pause && delay == 0 {
                    let n = app.pause_agents_now();
                    app.pause_from_off = n > 0;
                }
            }
        });
        if app.off_delay > 0 && app.quick_pause {
            ui.label(RichText::new("Agents are only paused for a break that starts now.").size(12.0).color(theme::pal().text_dim));
        }
        app.delayed_off_rows(ui);
    });
    ui.add_space(theme::SP_S);

    agents_card(app, ui, &mut s, &mut changed);
    ui.add_space(theme::SP_S);

    widgets::card(ui, "My break presets", |ui| {
        ui.label(RichText::new("Named breaks you can start with one click. Edit anything, add your own, delete the rest.").size(12.0).color(theme::pal().text_dim));
        let mut del = None;
        for p in s.presets.iter_mut() {
            ui.horizontal_wrapped(|ui| {
                changed |= ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(120.0).char_limit(24)).on_hover_text("Preset name").changed();
                changed |= super::drag_min(ui, &mut p.mins, 1..=720, " min");
                for (t, l) in [(BreakTarget::Mon1, "1"), (BreakTarget::Mon2, "2"), (BreakTarget::Both, "Both")] {
                    if widgets::chip(ui, l, p.target == t).on_hover_text("Which screens this preset turns off").clicked() {
                        p.target = t;
                        changed = true;
                    }
                }
                if widgets::chip(ui, "Pause", p.pause_agents).on_hover_text("Suspend running Claude Code agents during this break").clicked() {
                    p.pause_agents = !p.pause_agents;
                    changed = true;
                }
                if widgets::ghost_button(ui, "Delete").on_hover_text("Remove this preset").clicked() {
                    del = Some(p.id);
                }
            });
        }
        if let Some(id) = del {
            crate::engine::preset_delete(&mut s.presets, id);
            changed = true;
        }
        if s.presets.is_empty() {
            ui.label(RichText::new("No presets yet.").color(theme::pal().text_dim));
        }
        if widgets::ghost_button(ui, "+ Add preset").on_hover_text("Create a new named preset").clicked() && crate::engine::preset_add(&mut s.presets).is_some() {
            changed = true;
        }
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Presets", |ui| {
        let cur = preset_index(&s);
        ui.horizontal_wrapped(|ui| {
            for (i, (name, w, b)) in PRESETS.iter().enumerate() {
                if preset_card(ui, name, &format!("{w} min work, {} rest", super::fmt_dur(*b)), cur == Some(i)) {
                    s.work_min = *w;
                    s.break_secs = *b;
                    changed = true;
                }
            }
            preset_card(ui, "Custom", &format!("{} min / {}", s.work_min, super::fmt_dur(s.break_secs)), cur.is_none());
        });
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Rhythm", |ui| {
        changed |= widgets::setting_toggle(ui, &mut s.break_enabled, "Remind me to take breaks", "Turns the break timer on or off.");
        changed |= widgets::slider_row(ui, "Work for", &mut s.work_min, 1..=crate::engine::MAX_WORK_MIN, " min", "Time you work before a break starts.");
        changed |= widgets::slider_row(ui, "Break lasts", &mut s.break_secs, 5..=crate::engine::BREAK_MAX_SECS, " s", "How long each short break is.");
        changed |= widgets::slider_row(ui, "Warn me", &mut s.warn_secs, 0..=120, " s before", "A heads-up before the break; 0 = no warning.");
        changed |= widgets::slider_row(ui, "Long break every", &mut s.long_break_every, 0..=20, " breaks", "Every Nth break is a long one; 0 = never.");
        if s.long_break_every > 0 {
            changed |= widgets::slider_row(ui, "Long break lasts", &mut s.long_break_secs, 30..=crate::engine::BREAK_MAX_SECS, " s", "Length of the long break.");
        }
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "How a break looks", |ui| {
        let mut idx = match s.break_style {
            BreakStyle::Blackout => 0,
            BreakStyle::Dim => 1,
            BreakStyle::Notify => 2,
        };
        if widgets::segmented(ui, &mut idx, &["Blackout", "Dim", "Notify only"]) {
            s.break_style = [BreakStyle::Blackout, BreakStyle::Dim, BreakStyle::Notify][idx];
            changed = true;
        }
        ui.label(RichText::new("Blackout covers the screen, Dim dims it but stays click-through, Notify only shows a message and plays a sound.").size(12.0).color(theme::pal().text_dim));
        ui.add_space(theme::SP_S);
        let mut strict = !s.allow_skip;
        if widgets::setting_toggle(ui, &mut strict, "Strict mode", "A running break cannot be skipped. Snoozing before it starts still works.") {
            s.allow_skip = !strict;
            changed = true;
        }
        changed |= widgets::setting_toggle(ui, &mut s.idle_credit, "Count time away as a break", "If you leave the keyboard for a whole break, it counts as taken.");
        ui.label("Manual screen-off uses");
        let mut om = if s.off_mode == OffMode::Hardware { 0 } else { 1 };
        if widgets::segmented(ui, &mut om, &["Hardware off", "Blackout"]) {
            s.off_mode = if om == 0 { OffMode::Hardware } else { OffMode::Blackout };
            changed = true;
        }
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Snooze", |ui| {
        ui.label(RichText::new("Length of the \"my snooze\" button, tray menu and hotkey").size(12.0).color(theme::pal().text_dim));
        ui.horizontal_wrapped(|ui| {
            for m in [5u32, 10, 15, 30] {
                if widgets::chip(ui, &format!("{m} min"), s.snooze_mins == m).on_hover_text("Use this snooze length").clicked() {
                    s.snooze_mins = m;
                    changed = true;
                }
            }
            ui.label("Custom");
            changed |= super::drag_min(ui, &mut s.snooze_mins, 1..=120, " min");
        });
    });
    ui.add_space(theme::SP_S);
    if changed {
        *app.engine.settings_mut() = s;
        app.mark_dirty();
    }
}
