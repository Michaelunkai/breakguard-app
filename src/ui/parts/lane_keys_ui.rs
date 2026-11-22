//! slice lane_keys_ui (owned by one team agent)
//! API: pub fn poll_keys(ctx: &egui::Context, text_focused: bool) -> Vec<keys::Key>
//!      J -> Key::J, K -> Key::K, F -> Key::F, Space -> Key::Space, Escape -> Key::Escape.
//!      Returns empty when text_focused. Feed each key to keys::Nav::handle.
#![allow(dead_code)]

use crate::ui::logic::keys::Key;
use eframe::egui;

/// Map a single egui key to the logic key action; unknown keys -> None.
fn map_key(key: egui::Key) -> Option<Key> {
    match key {
        egui::Key::J => Some(Key::J),
        egui::Key::K => Some(Key::K),
        egui::Key::F => Some(Key::F),
        egui::Key::Space => Some(Key::Space),
        egui::Key::Escape => Some(Key::Escape),
        _ => None,
    }
}

/// Collect pressed (not repeated) lane-navigation keys this frame.
/// While a text field has focus, no keys are returned.
pub fn poll_keys(ctx: &egui::Context, text_focused: bool) -> Vec<Key> {
    if text_focused {
        return Vec::new();
    }
    ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } => map_key(*key),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, RawInput};


    fn press(key: egui::Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn run(events: Vec<Event>, text_focused: bool) -> Vec<Key> {
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let raw = RawInput {
            events,
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            out = poll_keys(ctx, text_focused);
        });
        out
    }

    #[test]
    fn j_gives_next_key() {
        assert_eq!(run(vec![press(egui::Key::J)], false), vec![Key::J]);
    }

    #[test]
    fn text_focused_gives_empty() {
        assert!(run(vec![press(egui::Key::J), press(egui::Key::Space)], true).is_empty());
    }

    #[test]
    fn mapping_k_f_space_escape() {
        assert_eq!(
            run(
                vec![
                    press(egui::Key::K),
                    press(egui::Key::F),
                    press(egui::Key::Space),
                    press(egui::Key::Escape),
                ],
                false
            ),
            vec![Key::K, Key::F, Key::Space, Key::Escape]
        );
    }

    #[test]
    fn unrelated_and_release_and_repeat_ignored() {
        let release = Event::Key {
            key: egui::Key::J,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let repeat = Event::Key {
            key: egui::Key::J,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        // egui recomputes `repeat` itself (a press after a release is a fresh press), so the
        // sequence is press J, then a repeat of the held J: only the first press counts.
        let got = run(vec![press(egui::Key::A), press(egui::Key::J), repeat, release], false);
        assert_eq!(got, vec![Key::J], "got {got:?}");
    }
}
