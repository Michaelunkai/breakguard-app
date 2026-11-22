//! Live Claude sessions dashboard: one card per session, filled from the background scanner.
use crate::sessions::{self, Env, SessionInfo, Status};
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
        ui.horizontal(|ui| {
            widgets::pulse_dot(ui, status_kind(s.status).color(), s.status == Status::Running);
            let name = if s.name.is_empty() { "(untitled)" } else { s.name.as_str() };
            ui.label(RichText::new(name).size(16.0).strong()).on_hover_text(&s.cwd);
            widgets::status_pill(ui, s.env.label(), env_kind(s.env));
            widgets::status_pill(ui, s.status.label(), status_kind(s.status));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::chip(ui, if *pinned { "Pinned" } else { "Pin" }, *pinned).on_hover_text("Keep this session at the top").clicked() {
                    *pinned = !*pinned;
                }
            });
        });
        if let Some((d, t)) = s.progress {
            ui.horizontal(|ui| {
                ui.add_sized([ui.available_width() - 70.0, 10.0], |ui: &mut Ui| widgets::progress_bar(ui, d as f32 / t.max(1) as f32, theme::accent(), 10.0));
                ui.label(RichText::new(format!("{d}/{t}")).color(theme::pal().text_dim));
            });
        }
        if !s.activity.is_empty() {
            ui.label(RichText::new(&s.activity).monospace().color(theme::pal().text));
        }
        if let Some(n) = &s.note {
            ui.label(RichText::new(n).size(12.0).color(theme::WARN));
        }
        let since = ((now_ms - s.last_active_ms) / 1000).max(0);
        let up = ((now_ms - s.created_ms) / 1000).max(0);
        ui.label(RichText::new(format!("{}  |  open for {}  |  {}", sessions::age_text(since), sessions::elapsed_text(up), s.cwd)).size(12.0).color(theme::pal().text_dim));
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
    ui.horizontal_wrapped(|ui| {
        use crate::ui::icons::Icon;
        widgets::stat_tile(ui, Icon::Play, &sum.running.to_string(), "Running", theme::accent());
        widgets::stat_tile(ui, Icon::Bell, &sum.waiting.to_string(), "Needs input", theme::WARN);
        widgets::stat_tile(ui, Icon::Check, &sum.done.to_string(), "Finished", theme::OK);
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
    sessions::sort_sessions(&mut all, &app.sess_pinned);
    let now_ms = sessions::now_ms();
    let mut shown = 0;
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
        empty_state(ui, all.is_empty());
    }
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
