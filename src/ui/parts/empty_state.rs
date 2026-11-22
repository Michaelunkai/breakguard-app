//! slice empty_state (owned by one team agent)
//! API: pub fn empty_lanes(ui: &mut Ui)
//! Centred empty-lane card: sleeping creature (kind 0, 64px), title, hint line.
#![allow(dead_code)]

use crate::ui::creatures::{self, Pose};
use crate::ui::theme;
use eframe::egui::{self, vec2, FontId, Layout, RichText, Ui};

pub const EMPTY_TITLE: &str = "No sessions yet";
pub const EMPTY_HINT: &str = "Start a Claude Code session and it appears here";
const CREATURE_SIZE: f32 = 64.0;

/// Pages that can show an empty or error state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyPage {
    Log,
    Stats,
    Sessions,
    Monitors,
    Schedule,
}

/// Title and hint for each page. Every entry must stay non-empty.
pub const PAGE_MESSAGES: [(EmptyPage, &str, &str); 5] = [
    (EmptyPage::Log, "No events yet", "Activity will be logged here as it happens"),
    (EmptyPage::Stats, "No stats yet", "Stats appear once a session has run"),
    (EmptyPage::Sessions, EMPTY_TITLE, EMPTY_HINT),
    (EmptyPage::Monitors, "No monitors yet", "Add a monitor to watch a process or window"),
    (EmptyPage::Schedule, "Nothing scheduled", "Create a schedule to run breaks automatically"),
];

pub fn page_message(page: EmptyPage) -> (&'static str, &'static str) {
    PAGE_MESSAGES
        .iter()
        .find(|(p, _, _)| *p == page)
        .map(|(_, t, h)| (*t, *h))
        .unwrap_or((EMPTY_TITLE, EMPTY_HINT))
}

/// Centred empty-state card for any page: sleeping creature, page title, hint line.
pub fn empty_page(ui: &mut Ui, page: EmptyPage) {
    let (title, hint) = page_message(page);
    let t = ui.input(|i| i.time);
    ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(CREATURE_SIZE, CREATURE_SIZE), egui::Sense::hover());
        creatures::draw(ui.painter(), 0, rect.center(), CREATURE_SIZE, Pose::Sleeping, t);
        ui.add_space(theme::SP_S);
        let pal = theme::pal();
        ui.label(RichText::new(title).font(FontId::proportional(16.0)).color(pal.text).strong());
        ui.label(RichText::new(hint).color(pal.text_dim));
    });
}

pub fn empty_lanes(ui: &mut Ui) {
    empty_page(ui, EmptyPage::Sessions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_lanes_renders_headless_without_panic() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, empty_lanes);
        });
    }

    #[test]
    fn empty_lanes_text_passes_aa_on_both_palettes() {
        use crate::ui::logic::contrast::passes_aa;
        use crate::ui::theme::{DARK_PAL, LIGHT_PAL};
        let rgb = |c: egui::Color32| (c.r(), c.g(), c.b());
        for pal in [DARK_PAL, LIGHT_PAL] {
            for fg in [pal.text, pal.text_dim] {
                for bg in [pal.bg, pal.surface, pal.surface_hi, pal.surface_lo] {
                    assert!(passes_aa(rgb(fg), rgb(bg)), "fg={:?} bg={:?}", rgb(fg), rgb(bg));
                }
            }
        }
    }

    #[test]
    fn hint_text_is_exact() {
        assert_eq!(EMPTY_HINT, "Start a Claude Code session and it appears here");
    }

    #[test]
    fn every_page_has_non_empty_title_and_hint() {
        for page in [EmptyPage::Log, EmptyPage::Stats, EmptyPage::Sessions, EmptyPage::Monitors, EmptyPage::Schedule] {
            let (title, hint) = page_message(page);
            assert!(!title.trim().is_empty(), "{page:?} title empty");
            assert!(!hint.trim().is_empty(), "{page:?} hint empty");
        }
    }

    #[test]
    fn message_table_covers_each_page_once() {
        assert_eq!(PAGE_MESSAGES.len(), 5);
        for (i, (p, _, _)) in PAGE_MESSAGES.iter().enumerate() {
            assert!(PAGE_MESSAGES.iter().skip(i + 1).all(|(q, _, _)| q != p));
        }
    }

    #[test]
    fn empty_page_renders_headless_without_panic() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| empty_page(ui, EmptyPage::Log));
        });
    }
}
