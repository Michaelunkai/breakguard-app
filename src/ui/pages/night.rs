use crate::ui::{widgets, App};
use eframe::egui::Ui;

fn hm(ui: &mut Ui, label: &str, v: &mut (u8, u8)) -> bool {
    let mut c = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let (mut h, mut m) = (v.0 as u32, v.1 as u32);
        c |= super::drag_min(ui, &mut h, 0..=23, " h");
        c |= super::drag_min(ui, &mut m, 0..=59, " m");
        *v = (h as u8, m as u8);
    });
    c
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Night mode");
    let mut s = app.engine.settings().clone();
    let mut c = false;
    widgets::card(ui, "Warm dim overlay", |ui| {
        c |= crate::ui::widgets::checkbox(ui, &mut s.night_enabled, "Turn on automatically at night").on_hover_text("Dims and warms the screen between the From and Until times").changed();
        c |= hm(ui, "From", &mut s.night_start);
        c |= hm(ui, "Until", &mut s.night_end);
        let mut lvl = s.night_level as u32;
        ui.horizontal(|ui| {
            ui.label("Strength");
            // Accent-filled track left of the handle, grey rail right of it.
            ui.visuals_mut().selection.bg_fill = crate::ui::theme::accent();
            c |= ui.add(eframe::egui::Slider::new(&mut lvl, 5..=90).suffix(" %").trailing_fill(true)).changed();
        });
        s.night_level = lvl as u8;
    });
    ui.add_space(6.0);
    let active = app.view().night_active;
    let mut toggled = false;
    let mut auto = false;
    widgets::card(ui, "Night mode right now", |ui| {
        let (rect, _) = ui.allocate_exact_size(eframe::egui::vec2(ui.available_width(), 120.0), eframe::egui::Sense::hover());
        let t = ui.input(|i| i.time);
        crate::ui::parts::night_art::night_scene(ui.painter(), rect, active, t);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            widgets::status_pill(ui, if active { "Night mode is on" } else { "Night mode is off" }, if active { widgets::PillKind::Ok } else { widgets::PillKind::Warn });
            let label = if active { "Turn night mode off" } else { "Turn night mode on" };
            if widgets::primary_button(ui, label).on_hover_text("Switch the warm dim overlay now, any time of day").clicked() {
                toggled = true;
            }
            if s.night_force != 0 && widgets::ghost_button(ui, "Follow the schedule").on_hover_text("Stop overriding; use the From and Until times again").clicked() {
                auto = true;
            }
        });
        if s.night_force != 0 {
            ui.label(eframe::egui::RichText::new("Set by hand: stays this way until you change it.").size(12.0).color(crate::ui::theme::pal().text_dim));
        }
    });
    if toggled {
        app.engine.night_toggle();
        app.mark_dirty();
        s.night_force = app.engine.settings().night_force;
    }
    if auto {
        app.engine.night_auto();
        app.mark_dirty();
        s.night_force = 0;
    }
    if c {
        s.night_force = app.engine.settings().night_force;
        *app.engine.settings_mut() = s;
        app.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{Clock, Engine, Settings};

    struct FixedClock(i64);
    impl Clock for FixedClock {
        fn now_unix(&self) -> i64 {
            self.0
        }
        fn secs_since_midnight(&self) -> u32 {
            self.0.rem_euclid(86400) as u32
        }
    }

    /// The ON/OFF button calls `night_toggle` and must flip the real `night_active` view state.
    #[test]
    fn night_button_toggles_real_setting() {
        let noon = 1_700_000_000 - 1_700_000_000 % 86400 + 12 * 3600;
        let mut e = Engine::new(Settings::default(), Box::new(FixedClock(noon)));
        let _ = e.tick();
        assert!(!e.view().night_active, "starts off at noon");
        e.night_toggle();
        let _ = e.tick();
        assert!(e.view().night_active, "first press turns on");
        assert_eq!(e.settings().night_force, 1);
        e.night_toggle();
        let _ = e.tick();
        assert!(!e.view().night_active, "second press turns off");
        assert_eq!(e.settings().night_force, 2);
        e.night_auto();
        assert_eq!(e.settings().night_force, 0, "Follow the schedule clears the override");
    }
}
