//! Dashboard card: edit the short-break length in any minutes, with an optional seconds part.
use eframe::egui::{self, RichText, Ui};

use crate::ui::widgets;
use crate::ui::{theme, App};
use crate::engine::BREAK_MAX_SECS;

/// Quick picks in minutes.
const QUICK_MIN: [u32; 6] = [1, 2, 5, 7, 10, 15];

/// Split seconds into (minutes, seconds).
pub fn split(secs: u32) -> (u32, u32) {
    (secs / 60, secs % 60)
}

/// Join minutes + seconds into a valid break length (clamped like Settings::clamp).
pub fn join(min: u32, sec: u32) -> u32 {
    min.saturating_mul(60).saturating_add(sec.min(59)).clamp(5, BREAK_MAX_SECS)
}

pub fn card(app: &mut App, ui: &mut Ui) {
    let cur = app.engine.settings().break_secs;
    let (mut m, mut s) = split(cur);
    let mut with_secs = s != 0 || ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("bd_secs")).unwrap_or(false));
    let mut new = cur;
    widgets::card(ui, "Break length", |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(super::fmt_dur(cur)).size(26.0).strong());
            ui.add_space(theme::SP_S);
            ui.label("each short break");
        });
        ui.add_space(theme::SP_S);
        ui.horizontal_wrapped(|ui| {
            for q in QUICK_MIN {
                if widgets::chip(ui, &format!("{q} min"), cur == q * 60).clicked() {
                    new = q * 60;
                }
            }
        });
        ui.add_space(theme::SP_S);
        ui.horizontal(|ui| {
            ui.label("Custom:");
            let mut ch = super::drag_min(ui, &mut m, 0..=BREAK_MAX_SECS / 60, " min");
            if widgets::toggle(ui, &mut with_secs, "seconds").changed() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("bd_secs"), with_secs));
                if !with_secs {
                    s = 0;
                    ch = true;
                }
            }
            if with_secs {
                ch |= ui
                    .add(egui::DragValue::new(&mut s).range(0..=59).suffix(" s"))
                    .on_hover_text("Extra seconds")
                    .changed();
            }
            if ch {
                new = join(m, s);
            }
        });
    });
    if new != cur {
        let st = app.engine.settings_mut();
        st.break_secs = new;
        st.clamp();
        app.engine.apply_settings_changed();
        app.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Settings;

    #[test]
    fn arbitrary_break_minutes() {
        for m in [1u32, 7, 13, 45, 60, 90, 180] {
            let secs = join(m, 0);
            assert_eq!(secs, m * 60);
            assert_eq!(split(secs), (m, 0));
            let mut s = Settings::default();
            s.break_secs = secs;
            s.clamp();
            assert_eq!(s.break_secs, m * 60, "{m} min must survive clamp");
            let back = Settings::parse(&s.serialize());
            assert_eq!(back.break_secs, m * 60, "{m} min must survive save/load");
        }
    }

    #[test]
    fn break_seconds_option() {
        assert_eq!(join(7, 30), 450);
        assert_eq!(split(450), (7, 30));
        assert_eq!(join(0, 3), 5);
        assert_eq!(join(0, 99), 59);
        assert_eq!(join(9999, 0), BREAK_MAX_SECS);
    }
}
