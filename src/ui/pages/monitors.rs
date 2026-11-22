use crate::engine::OffMode;
use crate::ui::{theme, widgets, App, Kind};
use eframe::egui::Ui;

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Monitors");
    widgets::card(ui, "How screens turn off", |ui| {
        let mut mode = app.engine.settings().off_mode;
        ui.horizontal(|ui| {
            crate::ui::widgets::radio(ui, &mut mode, OffMode::Hardware, "Hardware off (monitor power)").on_hover_text("Turns the monitor off with its own power command (DDC)");
            crate::ui::widgets::radio(ui, &mut mode, OffMode::Blackout, "Software blackout").on_hover_text("Covers the screen with a black window; works on every monitor");
        });
        ui.label("Hardware off falls back to blackout when the monitor refuses.");
        if mode != app.engine.settings().off_mode {
            app.engine.settings_mut().off_mode = mode;
            app.mark_dirty();
        }
    });
    ui.add_space(8.0);
    if !app.monitors_loaded {
        ui.label(eframe::egui::RichText::new("Detecting monitors...").color(theme::pal().text_dim));
        return;
    }
    if app.monitors.is_empty() {
        crate::ui::parts::empty_state::empty_page(ui, crate::ui::parts::empty_state::EmptyPage::Monitors);
        return;
    }
    let mons = app.monitors.clone();
    let view = app.view();
    widgets::card(ui, "Start in", |ui| {
        ui.label("How long from now the screen turns off: Now, or any time up to 24 hours.");
        app.off_delay_edit(ui, "monitors-delay");
    });
    ui.add_space(6.0);
    widgets::card(ui, "Turn-off length", |ui| {
        ui.label("How long a screen stays off: any length from 1 second to 24 hours.");
        app.off_len_edit(ui, "monitors");
    });
    let label = app.off_chosen_label();
    ui.add_space(6.0);
    let all: Vec<String> = mons.iter().map(|m| m.name.clone()).collect();
    if mons.len() > 1 && widgets::primary_button(ui, &label.replace("Turn off", "Turn off all screens")).on_hover_text("Turn every screen off after the chosen start time, for exactly the chosen length").clicked() {
        app.off_chosen(all);
    }
    app.delayed_off_rows(ui);
    ui.add_space(6.0);
    for (i, m) in mons.iter().enumerate() {
        widgets::card(ui, &format!("Screen {}{}", i + 1, if m.primary { " (main)" } else { "" }), |ui| {
            ui.horizontal(|ui| {
                let mut sel = app.is_selected(&m.name);
                if crate::ui::widgets::checkbox(ui, &mut sel, "Use for breaks").on_hover_text("Include this screen when a break turns screens off").changed() {
                    app.toggle_selected(&m.name);
                }
                super::short_label(ui, &m.name, 28);
            });
            let (power, kind) = match m.ddc_power {
                Some(1) => ("Power on".to_string(), widgets::PillKind::Ok),
                Some(v) => (format!("Power state {v}"), widgets::PillKind::Warn),
                None => ("Power not reported".into(), widgets::PillKind::Neutral),
            };
            ui.horizontal(|ui| {
                widgets::status_pill(ui, &power, kind);
                ui.label(eframe::egui::RichText::new(format!("{}x{} at {},{}", m.w, m.h, m.x, m.y)).color(theme::pal().text_dim));
            });
            let is_off = view.active_off.iter().any(|(t, _)| t.contains(&m.name));
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &label).on_hover_text("Turn this screen off after the chosen start time, for exactly the chosen length").clicked() {
                    app.off_chosen(vec![m.name.clone()]);
                }
                if is_off {
                    widgets::status_pill(ui, "Screen off", widgets::PillKind::Warn);
                    if widgets::ghost_button(ui, "Turn on now").on_hover_text("Wake this screen immediately").clicked() {
                        app.turn_on(vec![m.name.clone()]);
                        app.toast(Kind::Info, "Turning screen on");
                    }
                }
            });
        });
        ui.add_space(6.0);
    }
}
