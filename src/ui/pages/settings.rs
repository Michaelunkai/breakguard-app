use crate::engine::Settings;
use crate::ui::{theme, widgets, App, Kind};
use eframe::egui::{self, RichText, Ui};

/// Letter box for a hotkey (A-Z), shows `Ctrl+Alt+<letter>`.
fn key_box(ui: &mut Ui, label: &str, k: &mut char) -> bool {
    let mut s = k.to_string();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout([110.0, 24.0].into(), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(label);
        });
        ui.label(RichText::new("Ctrl+Alt+").color(theme::pal().text_dim));
        let r = ui.add(egui::TextEdit::singleline(&mut s).desired_width(26.0).char_limit(1)).on_hover_text("One letter A-Z");
        if r.changed() {
            if let Some(c) = s.chars().next().filter(|c| c.is_ascii_alphabetic()) {
                *k = c.to_ascii_uppercase();
                changed = true;
            }
        }
    });
    changed
}

/// Toggle plus minutes box for one micro reminder (0 = off).
fn micro_row(ui: &mut Ui, label: &str, help: &str, mins: &mut u32, default: u32) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut on = *mins > 0;
        if widgets::toggle(ui, &mut on, label).on_hover_text(help).changed() {
            *mins = if on { default } else { 0 };
            changed = true;
        }
        if *mins > 0 {
            ui.label("every");
            changed |= ui.add(egui::DragValue::new(mins).range(5..=480).suffix(" min")).on_hover_text("Minutes between reminders (5-480)").changed();
        }
    });
    changed
}

fn tips_editor(app: &mut App, ui: &mut Ui, s: &mut Settings) -> bool {
    let mut changed = false;
    changed |= widgets::setting_toggle(ui, &mut s.tips, "Show a tip on every break", "A short idea for your eyes or body is shown on the break screen.");
    let mut remove = None;
    for (i, t) in s.custom_tips.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(t);
            if widgets::ghost_button(ui, "Remove").on_hover_text("Delete this tip").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        s.custom_tips.remove(i);
        changed = true;
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.tip_draft).hint_text("Write your own tip").desired_width(300.0)).on_hover_text("Your own tips replace the built-in ones");
        if widgets::ghost_button(ui, "Add tip").on_hover_text("Add to your tips list (max 20)").clicked() {
            let t = crate::engine::clean_tip(&app.tip_draft);
            if !t.is_empty() && s.custom_tips.len() < crate::engine::MAX_TIPS {
                s.custom_tips.push(t);
                app.tip_draft.clear();
                changed = true;
            }
        }
    });
    if s.custom_tips.is_empty() {
        ui.label(RichText::new("Using the built-in tips. Add your own to replace them.").size(12.0).color(theme::pal().text_dim));
    }
    changed
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Settings");
    let mut s = app.engine.settings().clone();
    let mut changed = false;

    widgets::card(ui, "General", |ui| {
        if widgets::setting_toggle(ui, &mut s.autostart, "Start with Windows", "Starts BreakGuard when you sign in.") {
            if !crate::sysx::autostart_set(s.autostart) {
                s.autostart = !s.autostart;
                app.toast(Kind::Err, "Could not change the autostart entry");
            }
            changed = true;
        }
        changed |= widgets::setting_toggle(ui, &mut s.start_minimized, "Start minimized", "Opens hidden in the tray instead of showing the window.");
        changed |= widgets::setting_toggle(ui, &mut s.close_to_tray, "Close button hides to tray", "The X keeps BreakGuard running in the tray. Quit from the tray menu.");
        changed |= widgets::setting_toggle(ui, &mut s.sound, "Sound", "Short tones for the warning, break start, break end and welcome back.");
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Hotkeys", |ui| {
        changed |= widgets::setting_toggle(ui, &mut s.hotkeys, "Global hotkeys", "Work even when BreakGuard is in the background.");
        ui.add_enabled_ui(s.hotkeys, |ui| {
            changed |= key_box(ui, "Break now", &mut s.hk_break);
            changed |= key_box(ui, "Pause / resume", &mut s.hk_pause);
            changed |= key_box(ui, "Snooze", &mut s.hk_snooze);
        });
        if s.hotkeys {
            if let Err(e) = crate::tray::validate(s.hk_break, s.hk_pause, s.hk_snooze) {
                ui.label(RichText::new(e).size(12.0).color(theme::DANGER));
            }
            for f in crate::tray::failed_hotkeys() {
                ui.label(RichText::new(format!("{f} is already used by another program")).size(12.0).color(theme::DANGER));
            }
        }
        ui.label(RichText::new("In the window: Ctrl+B break now, Ctrl+P pause, Ctrl+1..9 pages.").size(12.0).color(theme::pal().text_dim));
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Do not disturb", |ui| {
        changed |= widgets::setting_toggle(ui, &mut s.dnd_fullscreen, "Hold breaks during fullscreen", "No break pops up over a game, video or presentation.");
        changed |= widgets::setting_toggle(ui, &mut s.hours_enabled, "Quiet hours: only remind during work hours", "Outside these hours the timer waits.");
        if s.hours_enabled {
            changed |= widgets::time_picker(ui, "From", &mut s.hours_start);
            changed |= widgets::time_picker(ui, "Until", &mut s.hours_end);
        }
        let mut idle = s.idle_pause_secs / 60;
        if widgets::slider_row(ui, "Pause when away", &mut idle, 0..=30, " min", "Freeze the countdown after this long without input; 0 = never.") {
            s.idle_pause_secs = idle * 60;
            changed = true;
        }
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Micro reminders", |ui| {
        changed |= micro_row(ui, "Drink water", "A small nudge to hydrate.", &mut s.micro_water, 45);
        changed |= micro_row(ui, "Stand up", "Get off the chair for a minute.", &mut s.micro_stand, 60);
        changed |= micro_row(ui, "Check posture", "Shoulders down, back straight.", &mut s.micro_posture, 30);
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Break tips", |ui| {
        changed |= tips_editor(app, ui, &mut s);
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Appearance", |ui| {
        ui.label("Accent colour");
        if let Some(i) = widgets::swatches(ui, s.accent) {
            s.accent = i;
            theme::set_accent(i);
            theme::apply(ui.ctx());
            changed = true;
        }
        ui.add_space(theme::SP_S);
        ui.label("Theme");
        ui.horizontal(|ui| {
            let mut light = s.light_theme;
            let dark_clicked = if light {
                widgets::ghost_button(ui, "Dark").on_hover_text("Dark palette").clicked()
            } else {
                widgets::primary_button(ui, "Dark").on_hover_text("Dark palette").clicked()
            };
            let light_clicked = if light {
                widgets::primary_button(ui, "Light").on_hover_text("Bright palette").clicked()
            } else {
                widgets::ghost_button(ui, "Light").on_hover_text("Bright palette").clicked()
            };
            if (dark_clicked || light_clicked) && light != light_clicked {
                light = light_clicked;
                s.light_theme = light;
                theme::set_light(light);
                theme::apply(ui.ctx());
                changed = true;
            }
        });
        ui.add_space(theme::SP_S);
        let mut scale = s.ui_scale;
        if widgets::slider_row(ui, "Interface size", &mut scale, 90..=150, " %", "Makes everything bigger or smaller.") {
            s.ui_scale = crate::logic::clamp_scale(scale);
            ui.ctx().set_zoom_factor(s.ui_scale as f32 / 100.0);
            changed = true;
        }
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Reset", |ui| {
        if app.confirm_reset_settings {
            ui.label("Restore every setting to its default? Stats are kept.");
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, "Reset all settings").clicked() {
                    s = crate::logic::reset_settings(&s);
                    theme::set_accent(s.accent);
                    theme::set_light(s.light_theme);
                    theme::apply(ui.ctx());
                    ui.ctx().set_zoom_factor(s.ui_scale as f32 / 100.0);
                    app.confirm_reset_settings = false;
                    app.toast(Kind::Info, "Settings reset to defaults");
                    changed = true;
                }
                if widgets::ghost_button(ui, "Keep my settings").clicked() {
                    app.confirm_reset_settings = false;
                }
            });
        } else if widgets::ghost_button(ui, "Reset all settings").on_hover_text("Go back to the factory defaults").clicked() {
            app.confirm_reset_settings = true;
        }
    });
    ui.add_space(theme::SP_S);

    super::about::show(app, ui);

    if changed {
        *app.engine.settings_mut() = s;
        app.mark_dirty();
    }
}
