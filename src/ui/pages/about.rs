use crate::ui::{theme, widgets, App};
use eframe::egui::{RichText, Ui};

/// Label + value + copy button on one row.
fn path_row(ui: &mut Ui, label: &str, value: String) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).color(theme::pal().text_dim));
        ui.label(RichText::new(&value).monospace());
        if widgets::ghost_button(ui, "Copy").on_hover_text("Copy to the clipboard").clicked() {
            ui.ctx().copy_text(value);
        }
    });
}

pub fn show(_app: &mut App, ui: &mut Ui) {
    widgets::card(ui, "About BreakGuard", |ui| {
        ui.label(RichText::new(format!("Version {}", crate::VERSION)).size(18.0).strong());
        ui.add_space(theme::SP_XS);
        ui.label("Protects your eyes and sleep: break reminders, timed screen-off, scheduled screen-off, timed PC sleep and a warm night overlay.");
        ui.add_space(theme::SP_S);
        widgets::status_pill(ui, "Never closes, kills or locks your programs", widgets::PillKind::Ok);
        if crate::sys::is_mock() {
            ui.add_space(theme::SP_XS);
            widgets::status_pill(ui, "Mock mode: no real monitor or sleep call", widgets::PillKind::Warn);
        }
    });
    ui.add_space(theme::SP_S);
    widgets::card(ui, "Files", |ui| {
        path_row(ui, "Settings folder", crate::engine::data_dir().display().to_string());
        path_row(ui, "Log file", super::log::log_path().display().to_string());
    });
    ui.add_space(theme::SP_S);
    widgets::card(ui, "Shortcuts", |ui| {
        ui.label("Ctrl+1..9 switch pages, Ctrl+B break now, Ctrl+P pause.");
        ui.label("Tab moves between controls, Space or Enter presses.");
    });
}
