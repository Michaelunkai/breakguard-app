use crate::ui::icons::Icon;
use crate::ui::{theme, widgets, App, Kind};
use eframe::egui::{self, vec2, Align2, RichText, Sense, Ui};

/// Seven bars (oldest left), today highlighted, goal line dashed.
fn week_bars(ui: &mut Ui, days: &[crate::stats::DayStats], today: u32, goal: u32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, true, "Breaks taken in the last 7 days"));
    let p = ui.painter();
    p.rect_filled(rect, theme::rad(theme::R_S), theme::SURFACE_LO);
    let max = days.iter().map(|d| d.taken).max().unwrap_or(0).max(goal).max(1) as f32;
    let area = rect.shrink2(vec2(16.0, 14.0));
    let slot = area.width() / days.len().max(1) as f32;
    let plot_h = area.height() - 22.0;
    let gy = area.top() + plot_h * (1.0 - goal as f32 / max);
    p.line_segment([egui::pos2(area.left(), gy), egui::pos2(area.right(), gy)], egui::Stroke::new(1.0_f32, theme::with_alpha(theme::OK, 140)));
    for (i, d) in days.iter().enumerate() {
        let x = area.left() + slot * i as f32;
        let h = (plot_h * d.taken as f32 / max).max(if d.taken > 0 { 4.0 } else { 2.0 });
        let bar = egui::Rect::from_min_size(egui::pos2(x + slot * 0.2, area.top() + plot_h - h), vec2(slot * 0.6, h));
        let col = if d.day == today { theme::accent() } else { theme::mix(theme::SURFACE_HI, theme::accent(), 0.45) };
        p.rect_filled(bar, theme::rad(4), if d.taken == 0 { theme::BORDER } else { col });
        if d.taken > 0 {
            p.text(egui::pos2(bar.center().x, bar.top() - 8.0), Align2::CENTER_CENTER, d.taken.to_string(), egui::FontId::proportional(12.0), theme::TEXT);
        }
        let label = if d.day == today { "Today".to_string() } else { weekday(d.day).to_string() };
        p.text(egui::pos2(bar.center().x, area.bottom() - 4.0), Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), theme::pal().text_dim);
    }
}

/// 1970-01-01 was a Thursday.
pub fn weekday(day: u32) -> &'static str {
    ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][(day % 7) as usize]
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Stats");
    let today = app.engine.today();
    let d = app.engine.stats().day(today);
    let goal = app.engine.settings().daily_goal;
    let streak = app.engine.stats().streak(today, 1);
    let week = app.engine.stats().last_days(today, 7);

    if app.engine.stats().all_days().iter().all(|d| d.taken == 0 && d.skipped == 0 && d.snoozed == 0) {
        widgets::card(ui, "Today", |ui| {
            crate::ui::parts::empty_state::empty_page(ui, crate::ui::parts::empty_state::EmptyPage::Stats);
        });
        return;
    }

    widgets::card(ui, "Today", |ui| {
        ui.horizontal_wrapped(|ui| {
            widgets::stat_tile(ui, Icon::Cup, &d.taken.to_string(), "Breaks taken", theme::accent());
            widgets::stat_tile(ui, Icon::Skip, &d.skipped.to_string(), "Skipped", theme::DANGER);
            widgets::stat_tile(ui, Icon::Bell, &d.snoozed.to_string(), "Snoozed", theme::WARN);
            widgets::stat_tile(ui, Icon::Moon, &format!("{} min", d.rest_secs / 60), "Rested", theme::OK);
        });
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Daily goal", |ui| {
        ui.horizontal(|ui| {
            widgets::stat_tile(ui, Icon::Flame, &format!("{streak} days"), "Streak", theme::WARN);
            ui.vertical(|ui| {
                ui.label(RichText::new(format!("{} of {} breaks", d.taken, goal)).size(18.0));
                widgets::progress_bar(ui, crate::logic::goal_fraction(d.taken, goal), theme::OK, 12.0);
                ui.label(RichText::new(crate::logic::greeting(d.taken, goal, streak)).color(theme::pal().text_dim));
            });
        });
        let mut g = goal;
        if widgets::slider_row(ui, "Goal per day", &mut g, 1..=50, " breaks", "How many breaks you want to take each day.") {
            app.engine.settings_mut().daily_goal = g;
            app.mark_dirty();
        }
    });
    ui.add_space(theme::SP_S);

    widgets::card(ui, "Last 7 days", |ui| week_bars(ui, &week, today, goal));
    ui.add_space(theme::SP_S);

    let (t, sk, sn) = week.iter().fold((0, 0, 0), |a, d| (a.0 + d.taken, a.1 + d.skipped, a.2 + d.snoozed));
    if let Some(h) = crate::logic::coach_hint(t, sk, sn) {
        widgets::card(ui, "Coach", |ui| {
            ui.label(RichText::new(h).size(15.0));
        });
        ui.add_space(theme::SP_S);
    }

    widgets::card(ui, "Data", |ui| {
        ui.horizontal(|ui| {
            if widgets::ghost_button(ui, "Export CSV").on_hover_text("Save all daily counters as a CSV file in the BreakGuard folder").clicked() {
                let text = crate::logic::stats_csv(app.engine.stats().all_days());
                let path = crate::engine::data_dir().join("breakguard-stats.csv");
                let _ = std::fs::create_dir_all(crate::engine::data_dir());
                match std::fs::write(&path, text) {
                    Ok(()) => app.toast(Kind::Ok, format!("Saved {}", path.display())),
                    Err(e) => app.toast(Kind::Err, format!("Could not save CSV: {e}")),
                }
            }
            if app.confirm_reset_stats {
                if widgets::danger_button(ui, "Really reset stats").on_hover_text("Deletes all counters and the streak").clicked() {
                    app.engine.stats_mut().clear();
                    app.confirm_reset_stats = false;
                    app.toast(Kind::Info, "Stats reset");
                }
                if widgets::ghost_button(ui, "Keep").clicked() {
                    app.confirm_reset_stats = false;
                }
            } else if widgets::ghost_button(ui, "Reset stats").on_hover_text("Start counting from zero").clicked() {
                app.confirm_reset_stats = true;
            }
        });
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn weekday_anchor() {
        assert_eq!(super::weekday(0), "Thu");
        assert_eq!(super::weekday(20_453), "Wed", "2025-12-31");
    }
}
