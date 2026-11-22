//! API: pub fn pulse_dot(ui: &Ui, c: Pos2, base: Color32, running: bool)
//! API: pub fn blink_cursor(ui: &Ui, at: Pos2, h: f32, color: Color32, running: bool)
//! Running: animates via crate::ui::logic::pulse and requests a repaint in the frame floor.
//! Idle: draws a static dot at 0.35 alpha / no cursor-off-state change, requests no repaint.
#![allow(dead_code)]

use eframe::egui::{Color32, Pos2, Rect, Ui, vec2};

use crate::ui::logic::pulse::{cursor_visible, next_repaint_ms, pulse};

const DOT_RADIUS: f32 = 3.5;
const CURSOR_WIDTH: f32 = 2.0;

fn now_ms(ui: &Ui) -> i64 {
    (ui.input(|i| i.time) * 1000.0) as i64
}

fn request_repaint(ui: &Ui, running: bool, any_anim: bool) {
    if let Some(ms) = next_repaint_ms(now_ms(ui), running, any_anim) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(ms));
    }
}

/// Pulsing status dot. Running: alpha follows `pulse`. Idle: fixed 0.35 alpha.
pub fn pulse_dot(ui: &Ui, c: Pos2, base: Color32, running: bool) {
    let alpha = pulse(now_ms(ui), running);
    ui.painter().circle_filled(c, DOT_RADIUS, base.gamma_multiply(alpha));
    // 8 fps is enough for a slow 1.4 s pulse and keeps idle CPU under the 1 s / 10 s budget
    if running {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(125));
    }
}

/// Blinking text cursor: vertical bar of height `h` at `at`. Idle: never drawn.
pub fn blink_cursor(ui: &Ui, at: Pos2, h: f32, color: Color32, running: bool) {
    let now = now_ms(ui);
    if cursor_visible(now, running) {
        let rect = Rect::from_min_size(at, vec2(CURSOR_WIDTH, h));
        ui.painter().rect_filled(rect, 0.0, color);
    }
    request_repaint(ui, running, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, RawInput, ViewportId};
    use std::time::Duration;

    /// Runs one headless frame drawing `f`, returns the root repaint delay.
    fn repaint_delay(f: impl FnMut(&mut Ui)) -> Duration {
        let ctx = egui::Context::default();
        let mut f = f;
        // First frame always asks for a layout-settling repaint; measure the second.
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f(ui));
        });
        let out = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f(ui));
        });
        out.viewport_output[&ViewportId::ROOT].repaint_delay
    }

    #[test]
    fn idle_dot_and_cursor_request_no_repaint() {
        let delay = repaint_delay(|ui| {
            let c = Pos2::new(10.0, 10.0);
            pulse_dot(ui, c, Color32::WHITE, false);
            blink_cursor(ui, c, 12.0, Color32::WHITE, false);
        });
        assert_eq!(delay, Duration::MAX, "idle must not request a repaint");
    }

    #[test]
    fn running_dot_requests_repaint() {
        let delay = repaint_delay(|ui| {
            pulse_dot(ui, Pos2::new(10.0, 10.0), Color32::WHITE, true);
        });
        assert!(delay < Duration::MAX, "running must request a repaint");
        assert!(delay <= Duration::from_millis(530));
    }
}
