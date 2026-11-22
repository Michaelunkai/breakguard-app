//! slice settings_groups (owned by one team agent)
//! API: pub fn group(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) -> Response
//! API: pub fn row(ui: &mut Ui, label: &str, hint: &str, control: impl FnOnce(&mut Ui)) -> Response
//! group: SURFACE_LO rounded frame, small-caps TEXT_DIM title, hairline BORDER divider, then body.
//! row: label (+ optional dim hint) left, control right-aligned on the same line.
#![allow(dead_code)]

use eframe::egui::{self, Layout, Response, RichText, Sense, Ui, vec2};

use crate::ui::theme::{self, BORDER, SP_S};
use crate::ui::widgets;

/// One settings card: title, hint, and the setting labels it holds (mirrors pages/settings.rs cards).
pub struct GroupSpec {
    pub title: &'static str,
    pub hint: &'static str,
    pub items: &'static [&'static str],
}

pub const GROUPS: &[GroupSpec] = &[
    GroupSpec { title: "General", hint: "Startup and window behaviour", items: &["Start with Windows", "Start minimized", "Close button hides to tray", "Sound"] },
    GroupSpec { title: "Hotkeys", hint: "Work even when BreakGuard is in the background", items: &["Global hotkeys", "Break now", "Pause / resume", "Snooze"] },
    GroupSpec { title: "Do not disturb", hint: "When breaks are held back", items: &["Hold breaks during fullscreen", "Quiet hours: only remind during work hours", "From", "Until", "Pause when away"] },
    GroupSpec { title: "Micro reminders", hint: "Short nudges between breaks", items: &["Drink water", "Stand up", "Check posture"] },
    GroupSpec { title: "Break tips", hint: "Ideas shown on the break screen", items: &["Show a tip on every break", "Custom tips"] },
    GroupSpec { title: "Appearance", hint: "Accent colour and size", items: &["Accent colour", "Interface size"] },
    GroupSpec { title: "Reset", hint: "Back to factory defaults", items: &["Reset all settings"] },
];

const TITLE_SIZE: f32 = 11.0;
const LABEL_SIZE: f32 = 14.0;
const HINT_SIZE: f32 = 12.0;
const HAIRLINE: f32 = 1.0;

pub fn group(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) -> Response {
    ui.scope(|ui| {
        widgets::card(ui, title, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HAIRLINE), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, BORDER);
            ui.add_space(SP_S);
            body(ui);
        });
    })
    .response
}

pub fn row(ui: &mut Ui, label: &str, hint: &str, control: impl FnOnce(&mut Ui)) -> Response {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).size(LABEL_SIZE).color(theme::pal().text));
            if !hint.is_empty() {
                ui.label(RichText::new(hint).size(HINT_SIZE).color(theme::pal().text_dim));
            }
        });
        ui.with_layout(Layout::right_to_left(egui::Align::Center), control);
    })
    .response
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{CentralPanel, Context, RawInput, Rect};
    use std::cell::Cell;
    use std::collections::HashSet;

    #[test]
    fn no_duplicate_items_across_groups() {
        let mut seen = HashSet::new();
        for g in GROUPS {
            for item in g.items {
                assert!(seen.insert(*item), "duplicate settings item: {item}");
            }
        }
    }

    #[test]
    fn every_group_is_non_empty_and_titled() {
        assert!(!GROUPS.is_empty());
        for g in GROUPS {
            assert!(!g.title.is_empty(), "group without title");
            assert!(!g.items.is_empty(), "group '{}' has no items", g.title);
        }
    }

    #[test]
    fn group_and_row_render_headless_and_group_is_taller_than_row() {
        let ctx = Context::default();
        let group_h = Cell::new(0.0_f32);
        let row_h = Cell::new(0.0_f32);
        let _ = ctx.run(RawInput::default(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                let g: Rect = group(ui, "Display", |ui| {
                    let r: Response = row(ui, "Theme", "Pick a palette", |ui| {
                        ui.label("Dark");
                    });
                    row_h.set(r.rect.height());
                })
                .rect;
                group_h.set(g.height());
            });
        });
        assert!(row_h.get() > 0.0, "row allocated no height");
        assert!(
            group_h.get() > row_h.get(),
            "group height {} must exceed row height {}",
            group_h.get(),
            row_h.get()
        );
    }

    #[test]
    fn row_without_hint_and_group_without_body_do_not_panic() {
        let ctx = Context::default();
        let _ = ctx.run(RawInput::default(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                group(ui, "", |_| {});
                row(ui, "Only label", "", |_| {});
            });
        });
    }
}
