//! slice lane_toolbar (owned by one team agent)
//! API: pub struct ToolbarOut { freeze, clear, copy, export: bool } (Default)
//! API: pub fn lane_toolbar(ui: &mut egui::Ui, query: &mut String, matches: usize, frozen: bool, hidden_new: usize) -> ToolbarOut
//! API: pub fn match_label(n: usize) -> String   ("no matches" | "1 match" | "n matches")
#![allow(dead_code)]

use eframe::egui::{self, RichText};

const BTN_TEXT: f32 = 12.0;

/// Which toolbar buttons were clicked this frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarOut {
    pub freeze: bool,
    pub clear: bool,
    pub copy: bool,
    pub export: bool,
}

/// Human label for a search match count.
pub fn match_label(n: usize) -> String {
    match n {
        0 => "no matches".to_string(),
        1 => "1 match".to_string(),
        n => format!("{n} matches"),
    }
}

/// Search box with match count, plus freeze/unfreeze, clear, copy, export buttons.
/// `hidden_new` (lines arriving while frozen) is shown next to the freeze button.
pub fn lane_toolbar(
    ui: &mut egui::Ui,
    query: &mut String,
    matches: usize,
    frozen: bool,
    hidden_new: usize,
) -> ToolbarOut {
    let mut out = ToolbarOut::default();
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(query)
                .hint_text("search")
                .desired_width(160.0),
        );
        if !query.trim().is_empty() {
            ui.label(RichText::new(match_label(matches)).size(BTN_TEXT));
        }
        let freeze_label = if frozen {
            if hidden_new > 0 {
                format!("Unfreeze (+{hidden_new})")
            } else {
                "Unfreeze".to_string()
            }
        } else {
            "Freeze".to_string()
        };
        out.freeze = crate::ui::widgets::ghost_button(ui, &freeze_label).clicked();
        out.clear = crate::ui::widgets::ghost_button(ui, "Clear").clicked();
        out.copy = crate::ui::widgets::ghost_button(ui, "Copy").clicked();
        out.export = crate::ui::widgets::ghost_button(ui, "Export").clicked();
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_label_branches() {
        assert_eq!(match_label(0), "no matches");
        assert_eq!(match_label(1), "1 match");
        assert_eq!(match_label(2), "2 matches");
        assert_eq!(match_label(1000), "1000 matches");
    }

    #[test]
    fn default_is_nothing_clicked() {
        let d = ToolbarOut::default();
        assert!(!d.freeze && !d.clear && !d.copy && !d.export);
    }

    #[test]
    fn headless_frame_without_click_returns_default() {
        let ctx = egui::Context::default();
        let mut query = String::from("err");
        let mut out = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                out = Some(lane_toolbar(ui, &mut query, 3, false, 0));
            });
        });
        assert_eq!(out, Some(ToolbarOut::default()));
        assert_eq!(query, "err");
    }

    #[test]
    fn headless_frame_frozen_without_click_returns_default() {
        let ctx = egui::Context::default();
        let mut query = String::new();
        let mut out = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                out = Some(lane_toolbar(ui, &mut query, 0, true, 5));
            });
        });
        assert_eq!(out, Some(ToolbarOut::default()));
    }
}
