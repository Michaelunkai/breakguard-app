//! slice schedule_cards (owned by one team agent)
//! API: pub fn daily_card(ui: &mut Ui, time: &str, days: &str, on: &mut bool) -> Response
//! API: pub fn once_card(ui: &mut Ui, when: &str, on: &mut bool) -> Response
//! Clicking anywhere on a card flips `on`. Icon, heading, subtitle, toggle pill and elevation
//! shadow are painted with theme tokens only (no fonts beyond theme::num/FontId, no assets).
#![allow(dead_code)]
use crate::ui::theme::{self, rad, ON_FILL, R_M, SP_M};
use eframe::egui::{self, epaint::Shadow, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui};

const CARD_H: f32 = 72.0;
const ICON_R: f32 = 16.0;
const TOGGLE_W: f32 = 44.0;
const TOGGLE_H: f32 = 24.0;

/// Elevation shadow drawn under a card (low, soft).
fn elevation(p: &egui::Painter, rect: Rect) {
    let shadow = Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(90) };
    p.add(shadow.as_shape(rect, rad(R_M)));
}

/// Shared card painter: returns the interaction response; flips `on` on click.
fn card(ui: &mut Ui, heading: &str, subtitle: &str, icon: fn(&egui::Painter, Pos2, bool), on: &mut bool) -> Response {
    let width = ui.available_width().max(1.0);
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(width, CARD_H), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let p = ui.painter_at(rect);
    let accent = theme::accent();
    let hovered = response.hovered();

    elevation(&p, rect);
    let fill = if hovered { theme::pal().surface_hi } else { theme::pal().surface };
    let stroke = if *on { Stroke::new(1.5_f32, accent) } else { Stroke::new(1.0_f32, theme::pal().border) };
    p.rect(rect, rad(R_M), fill, stroke, egui::StrokeKind::Inside);

    // icon well
    let icon_c = Pos2::new(rect.left() + SP_M + ICON_R, rect.center().y);
    let well = if *on { theme::with_alpha(accent, 48) } else { theme::pal().surface_hi };
    p.circle_filled(icon_c, ICON_R, well);
    icon(&p, icon_c, *on);

    // heading + subtitle
    let text_x = icon_c.x + ICON_R + SP_M;
    let heading_col = if *on { theme::pal().text } else { theme::pal().text_dim };
    p.text(Pos2::new(text_x, rect.top() + 16.0), Align2::LEFT_TOP, heading, theme::num(18.0), heading_col);
    p.text(Pos2::new(text_x, rect.top() + 42.0), Align2::LEFT_TOP, subtitle, FontId::proportional(12.0), theme::pal().text_dim);

    // toggle pill
    let tx = rect.right() - SP_M - TOGGLE_W;
    let ty = rect.center().y - TOGGLE_H / 2.0;
    let track = Rect::from_min_size(Pos2::new(tx, ty), egui::vec2(TOGGLE_W, TOGGLE_H));
    let (track_fill, knob_x, knob_col) = if *on {
        (accent, track.right() - TOGGLE_H / 2.0, ON_FILL)
    } else {
        (theme::pal().border_strong, track.left() + TOGGLE_H / 2.0, theme::pal().text)
    };
    p.rect_filled(track, rad(theme::R_L), track_fill);
    p.circle_filled(Pos2::new(knob_x, track.center().y), TOGGLE_H / 2.0 - 3.0, knob_col);

    response
}

/// Clock face icon (daily schedule).
fn clock_icon(p: &egui::Painter, c: Pos2, on: bool) {
    let col = if on { theme::accent() } else { theme::pal().text_dim };
    p.circle_stroke(c, 8.0, Stroke::new(1.5_f32, col));
    p.line_segment([c, c + egui::vec2(0.0, -5.0)], Stroke::new(1.5_f32, col));
    p.line_segment([c, c + egui::vec2(3.5, 0.0)], Stroke::new(1.5_f32, col));
}

/// Calendar icon (one-off schedule).
fn calendar_icon(p: &egui::Painter, c: Pos2, on: bool) {
    let col = if on { theme::accent() } else { theme::pal().text_dim };
    let body = Rect::from_center_size(c + egui::vec2(0.0, 1.0), egui::vec2(16.0, 14.0));
    p.rect_stroke(body, rad(3), Stroke::new(1.5_f32, col), egui::StrokeKind::Inside);
    p.line_segment(
        [Pos2::new(body.left(), body.top() + 4.0), Pos2::new(body.right(), body.top() + 4.0)],
        Stroke::new(1.5_f32, col),
    );
}

/// Recurring schedule card: `time` heading (e.g. "09:30"), `days` subtitle (e.g. "Mon-Fri").
pub fn daily_card(ui: &mut Ui, time: &str, days: &str, on: &mut bool) -> Response {
    card(ui, time, days, clock_icon, on)
}

/// One-off schedule card: `when` (e.g. "Tomorrow 14:00") shown as heading, with calendar icon.
pub fn once_card(ui: &mut Ui, when: &str, on: &mut bool) -> Response {
    card(ui, when, "One time only", calendar_icon, on)
}

/// Weekday labels, index 0 = Mon.
pub const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub const EMPTY_LIST_TEXT: &str = "No schedules yet";

/// Pure text model for one schedule card.
#[derive(Debug, Clone, PartialEq)]
pub struct CardModel {
    pub title: String,
    pub subtitle: String,
    pub chips: [(&'static str, bool); 7],
}

/// Weekday chips with on/off state (index 0 = Mon).
pub fn weekday_chips(days: [bool; 7]) -> [(&'static str, bool); 7] {
    std::array::from_fn(|i| (WEEKDAYS[i], days[i]))
}

/// Subtitle summary of selected days: "Every day", "Mon-Fri", "No days", or a joined list.
pub fn days_summary(days: [bool; 7]) -> String {
    let on = days.iter().filter(|d| **d).count();
    if on == 0 {
        return "No days".to_string();
    }
    if on == 7 {
        return "Every day".to_string();
    }
    if days[..5].iter().all(|d| *d) && !days[5] && !days[6] {
        return "Mon-Fri".to_string();
    }
    WEEKDAYS
        .iter()
        .zip(days.iter())
        .filter(|(_, d)| **d)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parse "HH:MM" into minutes since midnight. None if malformed or out of range.
pub fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Next run of a daily schedule as "Day HH:MM". `now_day` 0 = Mon, `now_min` minutes since midnight.
/// None when no days are selected or `time` is malformed.
pub fn daily_next_run(time: &str, days: [bool; 7], now_day: usize, now_min: u32) -> Option<String> {
    let t = parse_hhmm(time)?;
    (0..7usize).find_map(|off| {
        let day = (now_day + off) % 7;
        let later = off > 0 || t > now_min;
        (days[day] && later).then(|| format!("{} {}", WEEKDAYS[day], time))
    })
}

/// Model for a recurring schedule card.
pub fn daily_model(time: &str, days: [bool; 7]) -> CardModel {
    CardModel { title: time.to_string(), subtitle: days_summary(days), chips: weekday_chips(days) }
}

/// Model for a one-off schedule card. `at` and `now` are unix minutes; past times show "Done".
pub fn once_model(when: &str, at: i64, now: i64) -> CardModel {
    let subtitle = if at <= now { "Done" } else { "One time only" };
    CardModel { title: when.to_string(), subtitle: subtitle.to_string(), chips: weekday_chips([false; 7]) }
}

#[cfg(test)]
mod model_tests {
    use super::*;

    #[test]
    fn days_summary_variants() {
        assert_eq!(days_summary([false; 7]), "No days");
        assert_eq!(days_summary([true; 7]), "Every day");
        assert_eq!(days_summary([true, true, true, true, true, false, false]), "Mon-Fri");
        assert_eq!(days_summary([false, false, false, false, false, true, true]), "Sat, Sun");
        assert_eq!(days_summary([true, false, true, false, false, false, false]), "Mon, Wed");
    }

    #[test]
    fn chips_follow_days() {
        let c = weekday_chips([true, false, false, false, false, false, true]);
        assert_eq!(c[0], ("Mon", true));
        assert_eq!(c[1], ("Tue", false));
        assert_eq!(c[6], ("Sun", true));
    }

    #[test]
    fn parse_hhmm_bounds() {
        assert_eq!(parse_hhmm("09:30"), Some(570));
        assert_eq!(parse_hhmm("23:59"), Some(1439));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("12:60"), None);
        assert_eq!(parse_hhmm("nope"), None);
        assert_eq!(parse_hhmm(""), None);
    }

    #[test]
    fn daily_model_text() {
        let m = daily_model("09:30", [true, true, true, true, true, false, false]);
        assert_eq!(m.title, "09:30");
        assert_eq!(m.subtitle, "Mon-Fri");
        assert!(m.chips[4].1 && !m.chips[5].1);
    }

    #[test]
    fn daily_next_run_today_or_later() {
        let all = [true; 7];
        // Tue 08:00 now, schedule 09:30 daily -> today Tue
        assert_eq!(daily_next_run("09:30", all, 1, 480).as_deref(), Some("Tue 09:30"));
        // Tue 10:00 now -> wraps to Wed
        assert_eq!(daily_next_run("09:30", all, 1, 600).as_deref(), Some("Wed 09:30"));
        // only Sat selected, now Sun -> next Sat
        let sat = [false, false, false, false, false, true, false];
        assert_eq!(daily_next_run("07:00", sat, 6, 0).as_deref(), Some("Sat 07:00"));
    }

    #[test]
    fn daily_next_run_none_cases() {
        assert_eq!(daily_next_run("09:30", [false; 7], 0, 0), None);
        assert_eq!(daily_next_run("bad", [true; 7], 0, 0), None);
    }

    #[test]
    fn once_past_is_done() {
        let past = once_model("Yesterday 14:00", 100, 200);
        assert_eq!(past.subtitle, "Done");
        let future = once_model("Tomorrow 14:00", 300, 200);
        assert_eq!(future.subtitle, "One time only");
        let now = once_model("Now", 200, 200);
        assert_eq!(now.subtitle, "Done");
    }

    #[test]
    fn empty_list_text_constant() {
        assert_eq!(EMPTY_LIST_TEXT, "No schedules yet");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, RawInput};

    /// Headless context with the app fonts (theme::num needs the "bold" family).
    fn headless() -> Context {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        ctx
    }

    /// Run one headless frame that paints `f` inside a central panel.
    fn run_frame(ctx: &Context, mut f: impl FnMut(&mut Ui)) {
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| f(ui));
        });
    }

    #[test]
    fn daily_card_paints_on_and_off_without_panic() {
        let ctx = headless();
        for (time, days) in [("09:30", "Mon-Fri"), ("", ""), ("23:59", "Every day, a very long subtitle that goes on")] {
            for start in [false, true] {
                let mut on = start;
                run_frame(&ctx, |ui| {
                    daily_card(ui, time, days, &mut on);
                });
                assert_eq!(on, start, "no click, state must not change");
            }
        }
    }

    #[test]
    fn once_card_paints_on_and_off_without_panic() {
        let ctx = headless();
        for when in ["Tomorrow 14:00", "", "Äöü 日本語 2026-11-20"] {
            for start in [false, true] {
                let mut on = start;
                run_frame(&ctx, |ui| {
                    once_card(ui, when, &mut on);
                });
                assert_eq!(on, start);
            }
        }
    }

    #[test]
    fn cards_survive_narrow_width() {
        let ctx = headless();
        let mut on = false;
        run_frame(&ctx, |ui| {
            ui.set_max_width(1.0);
            daily_card(ui, "12:00", "Sat", &mut on);
            once_card(ui, "Now", &mut on);
        });
    }
}
