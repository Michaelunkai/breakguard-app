//! slice lane_actions_ui (owned by one team agent)
//! API: pub enum LaneAction { OpenFolder(String), CopyId(String) }
//! API: pub fn lane_actions(ui: &mut egui::Ui, cwd: &str, session_id: &str) -> `Option<LaneAction>`
//! API: pub fn run(action: &LaneAction, mock: bool) -> String
#![allow(dead_code)]

use crate::ui::logic::actions::{copy_id_action, open_folder_action, Action};
use eframe::egui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneAction {
    OpenFolder(String),
    CopyId(String),
}

/// Two small buttons: "Open folder" and "Copy ID".
/// Copy ID puts the id on the clipboard immediately. Returns the clicked action.
pub fn lane_actions(ui: &mut egui::Ui, cwd: &str, session_id: &str) -> Option<LaneAction> {
    let mut picked = None;
    ui.horizontal(|ui| {
        if let Action::OpenFolder(p) = open_folder_action(cwd) {
            if crate::ui::widgets::ghost_button(ui, "Open folder").clicked() {
                picked = Some(LaneAction::OpenFolder(p));
            }
        }
        if let Action::CopyText(t) = copy_id_action(session_id) {
            if crate::ui::widgets::ghost_button(ui, "Copy ID").clicked() {
                ui.ctx().copy_text(t.clone());
                picked = Some(LaneAction::CopyId(t));
            }
        }
    });
    picked
}

/// Describes the action. Opens Explorer only when `mock` is false and not under test.
/// Returns the description string (for the status line / tests).
pub fn run(action: &LaneAction, mock: bool) -> String {
    match action {
        LaneAction::OpenFolder(p) => {
            if !mock && !cfg!(test) {
                let _ = std::process::Command::new("explorer").arg(p).spawn();
            }
            format!("open folder: {}", p)
        }
        LaneAction::CopyId(id) => format!("copy id: {}", id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_open_describes_normalized_path() {
        let a = LaneAction::OpenFolder("C:\\a\\b".to_string());
        assert_eq!(run(&a, true), "open folder: C:\\a\\b");
    }

    #[test]
    fn mock_copy_describes_id() {
        let a = LaneAction::CopyId("sess-1".to_string());
        assert_eq!(run(&a, true), "copy id: sess-1");
    }

    #[test]
    fn test_build_never_spawns_explorer() {
        // cfg!(test) guard: even mock=false must not launch a process.
        let a = LaneAction::OpenFolder("C:\\".to_string());
        assert_eq!(run(&a, false), "open folder: C:\\");
    }

    #[test]
    fn open_target_from_cwd() {
        assert_eq!(
            open_folder_action("C:/proj/x/"),
            Action::OpenFolder("C:\\proj\\x".to_string())
        );
        assert_eq!(open_folder_action("  "), Action::None);
    }

    #[test]
    fn copy_target_from_id() {
        assert_eq!(copy_id_action("abc"), Action::CopyText("abc".to_string()));
        assert_eq!(copy_id_action(""), Action::None);
    }
}
