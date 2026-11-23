//! Live Claude sessions dashboard: one card per session, filled from the background scanner.
use crate::sessions::{self, Env, SessionInfo, Status};
use crate::ui::logic::cardview;
use crate::ui::widgets::{self, PillKind};
use crate::ui::{theme, App};
use eframe::egui::{self, vec2, RichText, Ui};
use std::time::Instant;

pub fn status_kind(s: Status) -> PillKind {
    match s {
        Status::Running => PillKind::Info,
        Status::WaitingInput => PillKind::Warn,
        Status::Error => PillKind::Danger,
        Status::Idle => PillKind::Neutral,
        Status::Finished => PillKind::Ok,
    }
}

fn env_kind(e: Env) -> PillKind {
    match e {
        Env::Cloud | Env::Remote => PillKind::Warn,
        Env::Unknown => PillKind::Neutral,
        _ => PillKind::Info,
    }
}

const CHIPS: [Status; 5] = [Status::Running, Status::WaitingInput, Status::Idle, Status::Finished, Status::Error];

/// Banner text when the data is not live, `None` when fresh.
pub fn not_live_text(snap: Option<&sessions::Snapshot>, now: Instant) -> Option<String> {
    match snap {
        None => Some("Not live: waiting for the first scan".into()),
        Some(s) if !s.ok => Some("Not live: the session folders cannot be read right now".into()),
        Some(s) if sessions::is_stale(s, now) => Some(format!("Not live: last update {} s ago", now.saturating_duration_since(s.taken).as_secs())),
        _ => None,
    }
}

fn card(ui: &mut Ui, s: &SessionInfo, pinned: &mut bool, now_ms: i64) {
    widgets::card(ui, "", |ui| {
        ui.horizontal_wrapped(|ui| {
            widgets::pulse_dot(ui, status_kind(s.status).color(), s.status == Status::Running);
            let name = if s.name.is_empty() { "(untitled)" } else { s.name.as_str() };
            let label = ui.add(egui::Label::new(RichText::new(name).size(16.0).strong()).truncate()).on_hover_text(&s.cwd);
            label.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, cardview::aria_label(&cardview::card_view(s, now_ms))));
            widgets::status_pill(ui, s.env.label(), env_kind(s.env));
            widgets::status_pill(ui, s.status.label(), status_kind(s.status));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::chip(ui, if *pinned { "Pinned" } else { "Pin" }, *pinned).on_hover_text("Keep this session at the top").clicked() {
                    *pinned = !*pinned;
                }
            });
        });
        let view = cardview::card_view(s, now_ms);
        if let Some(f) = view.progress {
            ui.horizontal(|ui| {
                let f = ui.ctx().animate_value_with_time(egui::Id::new(("sess-prog", &s.id)), f, cardview::progress_glide_secs(cardview::reduced_motion()));
                ui.add_sized([ui.available_width() - 70.0, 10.0], |ui: &mut Ui| widgets::progress_bar(ui, f, theme::accent(), 10.0));
                ui.label(RichText::new(&view.progress_text).color(theme::pal().text_dim));
            });
        }
        if !view.step.is_empty() {
            ui.label(RichText::new(&view.step).monospace().color(theme::pal().text));
        }
        if !view.last_line.is_empty() && view.last_line != view.step {
            ui.label(RichText::new(format!("> {}", view.last_line)).monospace().size(12.0).color(theme::pal().text_dim));
        }
        if let Some(n) = &s.note {
            ui.label(RichText::new(n).size(12.0).color(theme::WARN));
        }
        if !sessions::is_running(s) {
            ui.label(RichText::new(sessions::finished_summary(s, now_ms)).color(theme::pal().text));
        }
        let since = ((now_ms - s.last_active_ms) / 1000).max(0);
        ui.label(RichText::new(format!("{}  |  open for {}  |  {}", sessions::age_text(since), view.elapsed, s.cwd)).size(12.0).color(theme::pal().text_dim));
    });
    ui.add_space(theme::SP_XS);
}

/// Illustrated empty state: no sessions at all, or nothing matches the filters.
fn empty_state(ui: &mut Ui, none_at_all: bool) {
    let (title, hint) = if none_at_all {
        ("No Claude sessions yet", "Start a session in Claude Code and it appears here within a second.")
    } else {
        ("No sessions match", "Clear the search or state filters to see every session.")
    };
    ui.vertical_centered(|ui| {
        ui.add_space(theme::SP_L);
        let (rect, _) = ui.allocate_exact_size(vec2(120.0, 84.0), egui::Sense::hover());
        let p = ui.painter();
        let accent = theme::accent();
        let body = theme::mix(theme::pal().surface, accent, 0.12);
        // Terminal window: frame, title bar, prompt lines.
        let win = egui::Rect::from_min_size(rect.min + vec2(10.0, 6.0), vec2(100.0, 66.0));
        p.rect_filled(win, 8.0, body);
        p.rect_stroke(win, 8.0, egui::Stroke::new(1.5_f32, accent), egui::StrokeKind::Inside);
        let bar = egui::Rect::from_min_size(win.min, vec2(100.0, 14.0));
        p.rect_filled(bar, egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, theme::mix(body, accent, 0.25));
        for (i, c) in [theme::WARN, theme::OK, theme::pal().text_dim].iter().enumerate() {
            p.circle_filled(bar.left_center() + vec2(9.0 + i as f32 * 9.0, 0.0), 3.0, *c);
        }
        let mut y = win.top() + 26.0;
        for w in [56.0_f32, 38.0, 46.0] {
            p.rect_filled(egui::Rect::from_min_size(egui::pos2(win.left() + 12.0, y), vec2(w, 5.0)), 2.0, theme::pal().text_dim);
            y += 12.0;
        }
        p.circle_filled(egui::pos2(win.right() - 14.0, win.bottom() - 12.0), 4.0, accent);
        ui.add_space(theme::SP_S);
        ui.label(RichText::new(title).size(16.0).strong());
        ui.label(RichText::new(hint).color(theme::pal().text_dim));
        ui.add_space(theme::SP_L);
    });
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Sessions");
    let now = Instant::now();
    let snap = app.sessions_snapshot.clone();
    if let Some(msg) = not_live_text(snap.as_ref(), now) {
        widgets::banner(ui, theme::WARN, |ui| {
            ui.label(RichText::new(msg).color(theme::WARN));
        });
        ui.add_space(theme::SP_S);
    }
    let Some(snap) = snap else { return };
    let mut all: Vec<SessionInfo> = snap.sessions.iter().filter(|s| app.sess_show_archived || !s.archived).cloned().collect();
    let sum = sessions::summarize(&all);
    let tiles = sessions::tile_counts(&sum);
    ui.horizontal_wrapped(|ui| {
        use crate::ui::icons::Icon;
        widgets::stat_tile(ui, Icon::Play, &tiles.0.to_string(), "Running", theme::accent());
        widgets::stat_tile(ui, Icon::Bell, &sum.waiting.to_string(), "Needs input", theme::WARN);
        widgets::stat_tile(ui, Icon::Check, &tiles.1.to_string(), "Finished", theme::OK);
        widgets::stat_tile(ui, Icon::Sessions, &sum.total.to_string(), "Sessions", theme::pal().text_dim);
    });
    if sum.progress.1 > 0 {
        ui.label(RichText::new(format!("Overall progress {} of {} steps", sum.progress.0, sum.progress.1)).color(theme::pal().text_dim));
        widgets::progress_bar(ui, sum.progress.0 as f32 / sum.progress.1 as f32, theme::accent(), 10.0);
    }
    ui.add_space(theme::SP_S);
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.sess_query).hint_text("Search name, folder or activity").desired_width(240.0)).on_hover_text("Filter the sessions as you type");
        for st in CHIPS {
            let on = app.sess_chips.contains(&st);
            if widgets::chip(ui, st.label(), on).on_hover_text("Show only sessions in this state").clicked() {
                if on {
                    app.sess_chips.remove(&st);
                } else {
                    app.sess_chips.insert(st);
                }
            }
        }
        widgets::toggle(ui, &mut app.sess_show_archived, "Archived").on_hover_text("Include archived sessions");
        widgets::toggle(ui, &mut app.sess_notify, "Notify").on_hover_text("Toast and tray balloon when a session finishes or needs input");
    });
    ui.add_space(theme::SP_S);
    let n_running = sessions::running_sessions(&all).len();
    let n_finished = all.len() - n_running;
    app.sess_finished_tab = cardview::tab_nav(ui.ctx(), app.sess_finished_tab);
    ui.horizontal(|ui| {
        if ui.selectable_label(!app.sess_finished_tab, format!("Running ({n_running})")).clicked() {
            app.sess_finished_tab = false;
        }
        if ui.selectable_label(app.sess_finished_tab, format!("Finished ({n_finished})")).clicked() {
            app.sess_finished_tab = true;
        }
    });
    ui.add_space(theme::SP_S);
    let want_finished = app.sess_finished_tab;
    let none_total = all.is_empty();
    all.retain(|s| sessions::is_running(s) != want_finished);
    sessions::sort_for_tab(&mut all, &app.sess_pinned);
    let now_ms = sessions::now_ms();
    let mut shown = 0;
    // Cards fade in when the tab changes (instant under reduced motion).
    let tab_id = egui::Id::new("sess-tab-switch");
    let (last_tab, since) = ui.ctx().data_mut(|d| d.get_temp::<(bool, Instant)>(tab_id)).unwrap_or((want_finished, Instant::now() - std::time::Duration::from_secs(5)));
    let since = if last_tab != want_finished { Instant::now() } else { since };
    ui.ctx().data_mut(|d| d.insert_temp(tab_id, (want_finished, since)));
    let fade_ms = since.elapsed().as_millis() as i64;
    let fade = cardview::tab_fade(fade_ms, cardview::reduced_motion());
    if fade < 1.0 {
        ui.ctx().request_repaint();
    }
    ui.set_opacity(fade);
    for s in all.iter().filter(|s| sessions::matches(s, &app.sess_query, &app.sess_chips)) {
        let mut pinned = app.sess_pinned.contains(&s.id);
        card(ui, s, &mut pinned, now_ms);
        if pinned != app.sess_pinned.contains(&s.id) {
            if pinned {
                app.sess_pinned.insert(s.id.clone());
            } else {
                app.sess_pinned.remove(&s.id);
            }
        }
        shown += 1;
    }
    if shown == 0 {
        if !none_total && all.is_empty() {
            ui.label(RichText::new(cardview::empty_message(want_finished, false)).size(15.0).color(theme::pal().text_dim));
        } else {
            empty_state(ui, none_total);
        }
    }
    ui.set_opacity(1.0);
    ui.add_space(vec2(0.0, theme::SP_S).y);
    ui.label(RichText::new(format!("Updated {} ago, refreshes every second", sessions::age_text(now.saturating_duration_since(snap.taken).as_secs() as i64).trim_end_matches(" ago"))).size(12.0).color(theme::pal().text_dim));
}

/// Compact per-session tail for the Log page.
#[allow(dead_code)] // compact tail, Log page shows the full Sessions tab
pub fn log_tab(app: &mut App, ui: &mut Ui) {
    let Some(snap) = app.sessions_snapshot.clone() else {
        ui.label("No session data yet.");
        return;
    };
    let now_ms = sessions::now_ms();
    let mut v: Vec<&SessionInfo> = snap.sessions.iter().filter(|s| !s.archived && !s.activity.is_empty()).collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.last_active_ms));
    if v.is_empty() {
        ui.label(RichText::new("No session activity yet.").color(theme::pal().text_dim));
    }
    for s in v.iter().take(60) {
        ui.label(RichText::new(format!("{:>8}  {}  -  {}", sessions::age_text((now_ms - s.last_active_ms) / 1000), s.name, s.activity)).monospace());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{Env, Status};

    #[test]
    fn cards_never_overflow_horizontally_from_600_to_2560() {
        for w in [600.0_f32, 1280.0, 2560.0] {
            let ctx = egui::Context::default();
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(w, 900.0))), ..Default::default() };
            let mut used = 0.0_f32;
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for i in 0..3 {
                        let s = SessionInfo { id: format!("s{i}"), name: "A fairly long session title to stress wrapping behaviour".into(), cwd: "F:/a/very/long/folder/path/to/the/project".into(), env: Env::Desktop, status: Status::Running, archived: false, created_ms: 0, last_active_ms: 0, activity: "Bash: cargo test --release --all-features".into(), progress: Some((2, 4)), note: None, tail: vec![] };
                        let mut pinned = false;
                        card(ui, &s, &mut pinned, 3_600_000);
                    }
                    used = ui.min_rect().width();
                });
            });
            assert!(used <= w + 0.5, "content {used} wider than window {w}");
            assert!(used > w * 0.5, "content fills the width at {w}");
        }
    }
}
