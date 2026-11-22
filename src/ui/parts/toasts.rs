//! slice toasts (owned by one team agent)
//! API: pub enum ToastKind { Info, Success, Warn, Error }
//! API: pub fn toast(ctx: &egui::Context, id: &str, slot: usize, kind: ToastKind, msg: &str, age_ms: i64)
//! slot 0 = bottom-most card; each higher slot stacks upward by CARD_H + STACK_GAP.
#![allow(dead_code)]

use eframe::egui::{pos2, vec2, Align2, Area, Color32, Context, FontId, Id, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::ui::logic::ease::Transition;
use crate::ui::theme::{self, accent, DANGER, OK, WARN, R_M};

const CARD_W: f32 = 320.0;
const CARD_H: f32 = 56.0;
const MARGIN: f32 = 16.0;
const ACCENT_W: f32 = 4.0;
/// Bottom gap above the window edge so the lowest card clears the status bar.
const STATUS_CLEAR: f32 = 48.0;
const ICON_R: f32 = 11.0;
/// Vertical gap between stacked cards, px.
pub const STACK_GAP: f32 = 8.0;
/// Slide-in duration. Offset is 0 once `age_ms >= SLIDE_MS`.
pub const SLIDE_MS: i64 = 250;
/// Distance the card slides in from, in px (to the right of its resting spot).
pub const SLIDE_DISTANCE: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warn,
    Error,
}

/// Accent / icon colour per kind, all from theme tokens.
pub fn kind_color(kind: ToastKind) -> Color32 {
    match kind {
        ToastKind::Info => accent(),
        ToastKind::Success => OK,
        ToastKind::Warn => WARN,
        ToastKind::Error => DANGER,
    }
}

/// Max toasts shown at once; oldest are dropped first.
pub const STACK_LIMIT: usize = 4;
/// Auto-dismiss delay after creation, ms.
pub const TTL_MS: i64 = 4000;

/// True once `now_ms - created_ms >= TTL_MS`.
pub fn is_expired(created_ms: i64, now_ms: i64) -> bool {
    now_ms.saturating_sub(created_ms) >= TTL_MS
}

/// Keeps the newest `STACK_LIMIT` entries of `items` (ordered oldest→newest).
pub fn stack_limited<T>(items: &[T]) -> &[T] {
    &items[items.len().saturating_sub(STACK_LIMIT)..]
}

/// Remaining horizontal slide offset in px at `age_ms` (0 once slide finished).
pub fn slide_offset(age_ms: i64) -> f32 {
    Transition { start_ms: 0, dur_ms: SLIDE_MS }.offset_px(age_ms, SLIDE_DISTANCE)
}

/// Paints a bottom-right toast card. `id` keys the area; `age_ms` drives slide-in.
pub fn toast(ctx: &Context, id: &str, slot: usize, kind: ToastKind, msg: &str, age_ms: i64) {
    let dx = slide_offset(age_ms);
    let dy = -STATUS_CLEAR - slot as f32 * (CARD_H + STACK_GAP);
    let color = kind_color(kind);
    Area::new(Id::new(("toast", id)))
        .order(Order::Foreground)
        .interactable(false)
        .anchor(Align2::RIGHT_BOTTOM, vec2(-MARGIN + dx, dy))
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(CARD_W, CARD_H), Sense::hover());
            let p = ui.painter_at(rect);
            let pal = theme::pal();
            p.rect_filled(rect, R_M, pal.surface_hi);
            p.rect_stroke(rect, R_M, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
            let bar = Rect::from_min_size(rect.min, vec2(ACCENT_W, CARD_H));
            p.rect_filled(bar, 0, color);
            let icon_c = pos2(rect.left() + 24.0, rect.center().y);
            paint_icon(&p, icon_c, kind, color);
            p.text(
                pos2(rect.left() + 48.0, rect.center().y),
                Align2::LEFT_CENTER,
                msg,
                FontId::proportional(14.0),
                pal.text,
            );
        });
}

/// Mock-only demo: paints one card per kind, stacked (Info, Success, Warn, Error).
/// Wiring calls it when env BREAKGUARD_DEMO_TOASTS=1 is set.
pub fn demo(ctx: &Context, age_ms: i64) {
    let samples = [
        (ToastKind::Info, "Break starts in 2 minutes"),
        (ToastKind::Success, "Settings saved"),
        (ToastKind::Warn, "Monitor is not responding"),
        (ToastKind::Error, "Could not change autostart"),
    ];
    for (slot, (kind, msg)) in samples.iter().enumerate() {
        toast(ctx, &format!("demo-{slot}"), slot, *kind, msg, age_ms);
    }
}

/// Painter-drawn glyph: circle with info dot+bar, check, warning triangle, or cross.
fn paint_icon(p: &Painter, c: Pos2, kind: ToastKind, color: Color32) {
    p.circle_stroke(c, ICON_R, Stroke::new(1.5_f32, color));
    let s = Stroke::new(2.0_f32, color);
    match kind {
        ToastKind::Info => {
            p.circle_filled(pos2(c.x, c.y - 5.0), 1.6, color);
            p.line_segment([pos2(c.x, c.y - 1.5), pos2(c.x, c.y + 5.0)], s);
        }
        ToastKind::Success => {
            p.line_segment([pos2(c.x - 5.0, c.y), pos2(c.x - 1.5, c.y + 4.0)], s);
            p.line_segment([pos2(c.x - 1.5, c.y + 4.0), pos2(c.x + 5.5, c.y - 4.0)], s);
        }
        ToastKind::Warn => {
            let top = pos2(c.x, c.y - 6.0);
            let bl = pos2(c.x - 6.0, c.y + 5.0);
            let br = pos2(c.x + 6.0, c.y + 5.0);
            p.line_segment([top, bl], s);
            p.line_segment([bl, br], s);
            p.line_segment([br, top], s);
        }
        ToastKind::Error => {
            p.line_segment([pos2(c.x - 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y + 4.5)], s);
            p.line_segment([pos2(c.x + 4.5, c.y - 4.5), pos2(c.x - 4.5, c.y + 4.5)], s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_colours_distinct_per_kind() {
        let kinds = [ToastKind::Info, ToastKind::Success, ToastKind::Warn, ToastKind::Error];
        for i in 0..kinds.len() {
            for j in (i + 1)..kinds.len() {
                assert_ne!(kind_color(kinds[i]), kind_color(kinds[j]));
            }
        }
    }

    #[test]
    fn slide_offset_zero_after_250ms() {
        assert_eq!(slide_offset(SLIDE_MS), 0.0);
        assert_eq!(slide_offset(1000), 0.0);
        assert_eq!(slide_offset(0), SLIDE_DISTANCE);
        assert!(slide_offset(100) > 0.0 && slide_offset(100) < SLIDE_DISTANCE);
    }

    #[test]
    fn slide_offset_handles_negative_age() {
        assert_eq!(slide_offset(-5000), SLIDE_DISTANCE);
    }

    #[test]
    fn expires_at_ttl_with_fake_clock() {
        let created = 1_000;
        assert!(!is_expired(created, created));
        assert!(!is_expired(created, created + TTL_MS - 1));
        assert!(is_expired(created, created + TTL_MS));
        assert!(!is_expired(created, created - 500));
    }

    #[test]
    fn stack_keeps_newest_only() {
        let items: Vec<u32> = (0..10).collect();
        assert_eq!(stack_limited(&items), &[6, 7, 8, 9]);
        let few = [1u32, 2];
        assert_eq!(stack_limited(&few), &[1, 2]);
    }

    #[test]
    fn toast_paints_all_kinds_without_panic() {
        let ctx = Context::default();
        for kind in [ToastKind::Info, ToastKind::Success, ToastKind::Warn, ToastKind::Error] {
            let _ = ctx.run(Default::default(), |c| {
                toast(c, "t", 0, kind, "hello", 0);
                toast(c, "t", 1, kind, "hello", 300);
            });
        }
    }
}
