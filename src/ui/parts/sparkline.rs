//! slice sparkline (owned by one team agent)
//! API: pub fn sparkline(ui: &mut Ui, size: Vec2, line_times_ms: &[i64], now_ms: i64, color: Color32)
//! API: pub fn sparkline_model(line_times_ms: &[i64], now_ms: i64) -> `(Vec<f32>, f32)`  (pure: 30 bar heights 0..1, events per minute)
//! Draws 30 one-minute buckets (oldest left) as bars; tiny "N/min" label top-right.
#![allow(dead_code)]
use crate::ui::logic::spark::{buckets, normalize, per_minute};
use crate::ui::theme;
use eframe::egui::{Align2, Color32, CornerRadius, Pos2, Rect, Sense, Ui, Vec2};

const BUCKETS: usize = 30;
const BUCKET_MS: i64 = 60_000;
const LABEL_H: f32 = 11.0;

/// Pure model: normalized bar heights (oldest first) and events per minute over the window.
pub fn sparkline_model(line_times_ms: &[i64], now_ms: i64) -> (Vec<f32>, f32) {
    let counts = buckets(line_times_ms, now_ms, BUCKETS, BUCKET_MS);
    let rate = per_minute(&counts, BUCKET_MS);
    (normalize(&counts), rate)
}

pub fn sparkline(ui: &mut Ui, size: Vec2, line_times_ms: &[i64], now_ms: i64, color: Color32) {
    let (heights, rate) = sparkline_model(line_times_ms, now_ms);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);

    let label_h = LABEL_H.min(rect.height() * 0.5);
    let bars = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + label_h), rect.max);
    let slot = bars.width() / BUCKETS as f32;
    let bar_w = (slot - 1.0).max(1.0);
    if bars.height() > 0.0 && slot > 0.0 {
        for (i, &h) in heights.iter().enumerate() {
            if h <= 0.0 {
                continue;
            }
            let x = bars.left() + i as f32 * slot;
            let bh = (h * bars.height()).max(1.0);
            let bar = Rect::from_min_max(
                Pos2::new(x, bars.bottom() - bh),
                Pos2::new(x + bar_w, bars.bottom()),
            );
            painter.rect_filled(bar, CornerRadius::same(1), color);
        }
    }

    painter.text(
        Pos2::new(rect.right(), rect.top()),
        Align2::RIGHT_TOP,
        format!("{rate:.1}/min"),
        theme::mono(9.0),
        theme::pal().text_dim,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_all_zero() {
        let (h, rate) = sparkline_model(&[], 1_000_000);
        assert_eq!(h.len(), BUCKETS);
        assert!(h.iter().all(|&x| x == 0.0));
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn all_in_one_bucket() {
        let now = 1_000_000;
        // ages 0, 10s, 30s -> all in the newest bucket
        let (h, rate) = sparkline_model(&[now, now - 10_000, now - 30_000], now);
        assert_eq!(h.len(), BUCKETS);
        assert_eq!(h[BUCKETS - 1], 1.0);
        assert!(h[..BUCKETS - 1].iter().all(|&x| x == 0.0));
        assert!((rate - 0.1).abs() < 1e-6, "rate={rate}");
    }

    #[test]
    fn future_timestamps_ignored() {
        let now = 1_000;
        let (h, rate) = sparkline_model(&[now + 1, now + 5_000, i64::MAX], now);
        assert!(h.iter().all(|&x| x == 0.0));
        assert_eq!(rate, 0.0);
    }
}
