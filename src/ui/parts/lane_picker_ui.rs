//! slice lane_picker_ui (owned by one team agent)
//! API: `pub fn creature_picker(ui: &mut egui::Ui, current: usize) -> Option<usize>`
//! API: pub fn grid_cols(count: usize, width: f32) -> usize
//! Uses crate::ui::creatures::{COUNT, NAMES, Pose, draw} and crate::ui::theme::accent.
#![allow(dead_code)]
use crate::ui::{creatures, theme};
use eframe::egui::{self, Sense, Stroke};

const CELL: f32 = 28.0;
const GAP: f32 = 6.0;

/// Columns that fit `width` px with `CELL`-px cells and `GAP` gaps, clamped to 1..=count.
/// Returns 0 only when `count` is 0; non-finite or narrow widths give 1.
pub fn grid_cols(count: usize, width: f32) -> usize {
    if count == 0 {
        return 0;
    }
    if !width.is_finite() || width < CELL {
        return 1;
    }
    let fit = ((width + GAP) / (CELL + GAP)).floor() as usize;
    fit.clamp(1, count)
}

/// Grid of all creatures at 28 px with name tooltips. Returns the index clicked this frame.
pub fn creature_picker(ui: &mut egui::Ui, current: usize) -> Option<usize> {
    let cols = grid_cols(creatures::COUNT, ui.available_width());
    if cols == 0 {
        return None;
    }
    let t = ui.input(|i| i.time);
    let mut clicked = None;
    for row_start in (0..creatures::COUNT).step_by(cols) {
        ui.horizontal(|ui| {
            for i in row_start..(row_start + cols).min(creatures::COUNT) {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(CELL, CELL), Sense::click());
                let centre = rect.center();
                creatures::draw(ui.painter(), i, centre, CELL, creatures::Pose::Idle, t);
                if i == current {
                    ui.painter()
                        .circle_stroke(centre, CELL * 0.5, Stroke::new(2.0_f32, theme::accent()));
                }
                if resp.on_hover_text(creatures::NAMES[i]).clicked() {
                    clicked = Some(i);
                }
            }
        });
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{CentralPanel, Context, RawInput};

    #[test]
    fn grid_cols_zero_count_is_zero() {
        assert_eq!(grid_cols(0, 500.0), 0);
    }

    #[test]
    fn grid_cols_narrow_or_bad_width_is_one() {
        assert_eq!(grid_cols(12, 0.0), 1);
        assert_eq!(grid_cols(12, 27.9), 1);
        assert_eq!(grid_cols(12, -5.0), 1);
        assert_eq!(grid_cols(12, f32::NAN), 1);
        assert_eq!(grid_cols(12, f32::NEG_INFINITY), 1);
    }

    #[test]
    fn grid_cols_fits_exact_multiples() {
        // n cells need n*28 + (n-1)*6 px
        assert_eq!(grid_cols(12, 28.0), 1);
        assert_eq!(grid_cols(12, 62.0), 2);
        assert_eq!(grid_cols(12, 96.0), 3);
        assert_eq!(grid_cols(12, 95.9), 2);
    }

    #[test]
    fn grid_cols_clamped_to_count() {
        assert_eq!(grid_cols(12, 10_000.0), 12);
        assert_eq!(grid_cols(3, f32::INFINITY), 1);
        assert_eq!(grid_cols(3, 1.0e9), 3);
    }

    #[test]
    fn headless_frame_without_click_returns_none() {
        let ctx = Context::default();
        let mut out = Some(usize::MAX);
        let _ = ctx.run(RawInput::default(), |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                out = creature_picker(ui, 0);
            });
        });
        assert_eq!(out, None);
    }
}
