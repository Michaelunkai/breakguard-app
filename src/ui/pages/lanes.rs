//! Live log lanes: one full-width horizontal lane per running session, stacked vertically, each
//! showing that session's real-time output. Data comes from `logs::LogHub` (polled while open).
use crate::logs::{LineKind, LogLine, LogState, SessionLog};
use crate::ui::{theme, App};
use eframe::egui::{self, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use crate::ui::creatures;
use crate::ui::logic::{badge, clear, filter, fontsize, freeze, height, keys, layout::LaneLayout, order::{self, SortMode}, persist::LayoutSave, scroll::FollowState};
use crate::ui::parts::{error_badge, lane_keys_ui, lane_persist_io, lane_retention_ui};
#[path = "../parts/lane_a11y.rs"]
mod lane_a11y;
use std::collections::HashMap;

pub const LANE_H: f32 = 340.0;
/// Height of the stretch grip under every lane.
pub const GRIP_H: f32 = 22.0;
/// Left gutter that keeps the error glyph clear of the time column.
const ERROR_GUTTER: f32 = 20.0;
pub const TAG_W: f32 = 96.0;
/// Default time column mode (index into the "time: off / ago / clock" labels).
pub const DEFAULT_TIME_MODE: u8 = 1;
/// Minimum row height as a multiple of the font size (readability).
pub const MIN_LINE_RATIO: f32 = 1.5;

/// Per-lane view state (not part of the log data).
#[derive(Clone, Debug)]
pub struct LaneView {
    pub height: f32,
    pub follow: FollowState,
    pub freeze: freeze::View,
    pub cleared: clear::Cleared,
    /// Click on a row toggles this lane between truncated and fully wrapped rows.
    pub expanded: bool,
    /// Height measured the last time the lane was drawn (placeholder size when it is off-screen).
    pub last_h: f32,
    /// Short feedback after copy / export / open (shown beside the lane buttons).
    pub note: String,
    /// Expanded tool-group indices in the transcript feed.
    pub groups_open: std::collections::HashSet<usize>,
    /// Error badge click: show only errors.
    pub errors_only: bool,
    /// total_lines when the user last sat at the bottom (for the 'N new lines' pill).
    pub seen_total: u64,
}

impl Default for LaneView {
    fn default() -> Self {
        LaneView { height: LANE_H, follow: FollowState::default(), freeze: Default::default(), cleared: Default::default(), expanded: std::env::var("BREAKGUARD_LANES_EXPAND").is_ok(), last_h: 0.0, note: String::new(), groups_open: Default::default(), errors_only: false, seen_total: 0 }
    }
}

pub struct LaneUi {
    pub views: HashMap<String, LaneView>,
    pub layout: LaneLayout,
    pub sort: SortMode,
    pub filter: String,
    pub font: f32,
    pub time_mode: u8,
    pub dense: bool,
    pub wrap: bool,
    /// per-session creature picks, persisted to creature-overrides.txt in the data dir
    pub overrides: crate::ui::logic::picker::Overrides,
    /// keyboard selection (J/K move, Space freezes, F focuses filter)
    pub nav: keys::Nav,
    /// unseen-error badge state per lane
    pub badges: badge::Badges,
    /// first frame each finished lane was seen finished (drives dimming and removal)
    pub finished_at: HashMap<String, i64>,
    /// last layout written to disk; saved again only when it changes
    pub saved: LayoutSave,
}
/// Minutes a finished lane stays visible before it is removed (dimmed for the last 20 s).

impl Default for LaneUi {
    fn default() -> Self {
        let saved = lane_persist_io::load(&layout_path());
        let mut views = HashMap::new();
        for (id, h) in &saved.heights {
            views.entry(id.clone()).or_insert_with(LaneView::default).height = *h;
        }
        let layout = LaneLayout { collapsed: saved.collapsed.iter().cloned().collect(), solo: None };
        let sort = match saved.sort { 1 => SortMode::Name, 2 => SortMode::Newest, _ => SortMode::RunningFirst };
        let filter = std::env::var("BREAKGUARD_LANES_FILTER").unwrap_or_else(|_| saved.filter.clone());
        LaneUi { views, layout, sort, filter, font: fontsize::DEFAULT, time_mode: DEFAULT_TIME_MODE, dense: std::env::var("BREAKGUARD_LANES_DENSE").is_ok(), wrap: wrap_default(std::env::var_os("BREAKGUARD_LANES_NOWRAP").is_some()), overrides: load_overrides(), nav: keys::Nav::default(), badges: badge::Badges::default(), finished_at: HashMap::new(), saved }
    }
}

pub fn overrides_path() -> std::path::PathBuf {
    crate::engine::data_dir().join("creature-overrides.txt")
}

pub fn layout_path() -> std::path::PathBuf {
    crate::engine::data_dir().join("lanes-layout.txt")
}

/// Snapshot of the persisted parts of the lane page (collapsed lanes, custom heights, filter, sort).
fn snapshot(ui: &LaneUi) -> LayoutSave {
    let mut collapsed: Vec<String> = ui.layout.collapsed.iter().cloned().collect();
    collapsed.sort();
    let mut heights: Vec<(String, f32)> = ui.views.iter().filter(|(_, v)| (v.height - LANE_H).abs() > f32::EPSILON).map(|(k, v)| (k.clone(), v.height)).collect();
    heights.sort_by(|a, b| a.0.cmp(&b.0));
    let sort = match ui.sort { SortMode::RunningFirst => 0, SortMode::Name => 1, SortMode::Newest => 2 };
    LayoutSave { collapsed, heights, filter: ui.filter.clone(), sort }
}

pub fn load_overrides() -> crate::ui::logic::picker::Overrides {
    std::fs::read_to_string(overrides_path()).map(|t| crate::ui::logic::picker::Overrides::from_text(&t)).unwrap_or_default()
}

fn save_overrides(o: &crate::ui::logic::picker::Overrides) {
    let p = overrides_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, o.to_text());
}

/// Creature accents are tuned for the dark lane surface; in light mode darken them until they reach AA (4.5:1) as text.
pub fn accent_text(accent: Color32, light: bool) -> Color32 {
    if !light {
        return accent;
    }
    use crate::ui::logic::contrast::ratio;
    let bg = theme::LIGHT_PAL.surface_lo;
    let bgc = (bg.r(), bg.g(), bg.b());
    let mut f = 1.0f32;
    let mut c = accent;
    while f > 0.0 && ratio((c.r(), c.g(), c.b()), bgc) < 4.5 {
        f -= 0.05;
        c = Color32::from_rgb((accent.r() as f32 * f) as u8, (accent.g() as f32 * f) as u8, (accent.b() as f32 * f) as u8);
    }
    c
}

pub fn kind_color(k: LineKind) -> Color32 {
    kind_color_in(k, theme::pal(), theme::is_light())
}

/// Line color for a kind on the given palette; `light` picks the AA-tuned status colors.
pub fn kind_color_in(k: LineKind, _p: theme::Pal, light: bool) -> Color32 {
    use crate::ui::logic::palette::{line_rgb, LineKind as Pk, ThemeKind};
    let pk = match k {
        LineKind::Assistant => Pk::Assistant,
        LineKind::ToolCall => Pk::Tool,
        LineKind::ToolResult => Pk::Result,
        LineKind::User => Pk::User,
        LineKind::Error => Pk::Error,
    };
    let (r, g, b) = line_rgb(pk, if light { ThemeKind::Light } else { ThemeKind::Dark });
    Color32::from_rgb(r, g, b)
}

pub fn kind_tag(l: &LogLine) -> String {
    match l.kind {
        LineKind::Assistant => "claude says".into(),
        LineKind::ToolCall => tool_word(&l.tool).into(),
        LineKind::ToolResult => "done".into(),
        LineKind::User => "you".into(),
        LineKind::Error => "error".into(),
    }
}

/// Plain word for a tool call tag. Raw ids (mcp__x__y, unknown names) never show.
pub fn tool_word(tool: &str) -> &'static str {
    match tool {
        "Bash" => "Command",
        "Read" => "Read",
        "Edit" | "Write" | "MultiEdit" => "Edit",
        "Grep" | "Glob" => "Search",
        "WebFetch" | "WebSearch" => "Web",
        _ => "Tool",
    }
}

pub fn state_color(s: LogState) -> Color32 {
    let light = theme::is_light();
    match s {
        LogState::Running => if light { theme::LIGHT_OK } else { theme::OK },
        LogState::Idle => if light { theme::LIGHT_WARN } else { theme::WARN },
        LogState::Finished => theme::pal().text_dim,
    }
}

pub fn state_label(s: LogState) -> &'static str {
    match s {
        LogState::Running => "running",
        LogState::Idle => "idle",
        LogState::Finished => "finished",
    }
}

/// Single-line text for a row: control characters flattened, cut with an ellipsis to `max_chars`.
pub fn one_line(text: &str, max_chars: usize) -> String {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    crate::logs::truncate(flat.trim(), max_chars.max(2))
}

/// Wrap is on by default; `nowrap_env` (BREAKGUARD_LANES_NOWRAP set) turns it off at startup.
pub fn wrap_default(nowrap_env: bool) -> bool {
    !nowrap_env
}

/// Row height: spacing row height, never below MIN_LINE_RATIO times the font size.
pub fn min_row_h(font: f32, dense: bool) -> f32 {
    let p = crate::ui::logic::spacing::Prefs { density: if dense { crate::ui::logic::spacing::Density::Dense } else { crate::ui::logic::spacing::Density::Comfortable }, wrap: true, time_mode: 0, font };
    crate::ui::logic::spacing::row_height(&p).max(font * MIN_LINE_RATIO)
}

/// Shown text for one row. Wrap on: full sentence. Wrap off: cut only past `max_chars`.
pub fn shown_text(l: &LogLine, wrap: bool, max_chars: usize) -> String {
    let s = row_display(l, wrap);
    if wrap { s } else { one_line(&s, max_chars) }
}

/// Jump-to-live button rect, clamped inside the lane's output rect.
pub fn jump_rect(lane: Rect) -> Rect {
    let (w, h) = (100.0f32.min(lane.width()), 24.0f32.min(lane.height()));
    let x = (lane.max.x - 110.0).max(lane.min.x).min(lane.max.x - w);
    let y = (lane.max.y - 30.0).max(lane.min.y).min(lane.max.y - h);
    Rect::from_min_size(egui::pos2(x, y), vec2(w, h))
}

/// Row text: a tool call drops its `Name: ` prefix because the tag already shows the tool.
pub fn row_text(l: &LogLine) -> &str {
    if l.kind == LineKind::ToolCall {
        let t = l.text.strip_prefix(l.tool.as_str()).unwrap_or(&l.text);
        return t.strip_prefix(": ").unwrap_or(t);
    }
    &l.text
}

/// Sentence shown first on tool rows (plain English), raw command / output follows beside it.
pub fn row_plain(l: &LogLine) -> &str {
    if matches!(l.kind, LineKind::ToolCall | LineKind::ToolResult | LineKind::Error) { &l.plain } else { "" }
}

/// Text a row paints and measures. Default (`expanded` false): plain sentence only, raw text
/// hidden (it lives in the tooltip and expanded view). Expanded: plain, separator, raw.
/// Rows with no plain sentence (chat lines) always show their raw text.
pub fn row_display(l: &LogLine, expanded: bool) -> String {
    let p = row_plain(l);
    if p.is_empty() {
        row_text(l).to_string()
    } else if expanded {
        format!("{p}  \u{b7}  {}", row_text(l))
    } else {
        p.to_string()
    }
}

/// Minimum width of one side-by-side lane column.
pub const LANE_MIN_W: f32 = 420.0;

/// Columns for the lane page: side by side while each column keeps LANE_MIN_W, else 1.
pub fn lane_cols(n: usize, avail_w: f32) -> usize {
    crate::ui::logic::grid::columns_for(n, avail_w, LANE_MIN_W, theme::SP_S)
}

/// A lane paints only inside its own rect, never outside the parent clip.
pub fn lane_clip(rect: Rect, parent: Rect) -> Rect {
    rect.intersect(parent)
}

/// Extra margin around the viewport in which off-screen lanes are still drawn.
pub fn lane_cull_margin() -> f32 {
    240.0
}

/// Unique scroll-area id salt per lane id.
pub fn scroll_salt(id: &str) -> String {
    format!("lane:{id}")
}

fn draw_row(ui: &mut Ui, l: &LogLine, mono: &FontId, char_w: f32, rh: f32, now_ms: i64, time_mode: u8, query: &str, wrap: bool, line_h: f32, burst: bool) -> bool {
    use crate::ui::parts::{error_line, time_col, tokens_render, tool_chips};
    let row_h = if wrap {
        let tw0 = time_col::column_width(time_mode, char_w * 0.85);
        let x0 = 8.0 + if tw0 > 0.0 { tw0 + 8.0 } else { 0.0 } + TAG_W;
        let cols = (((ui.available_width() - x0 - 14.0).max(40.0)) / char_w) as usize;
        crate::ui::parts::lane_wrap_row::row_height(&row_display(l, true), cols, true, false, line_h).max(rh)
    } else {
        rh
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
    let clicked = resp.clicked();
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let age = if l.ts_ms > 0 { now_ms - l.ts_ms } else { i64::MAX };
    let (fade_a, fade_dy) = if age < 450 {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        crate::ui::parts::fade_in::row_anim(age.max(0), burst)
    } else {
        (1.0, 0.0)
    };
    let c = kind_color(l.kind).gamma_multiply(fade_a);
    if l.kind == LineKind::Error {
        error_line::paint_error_bar(ui.painter(), rect);
    }
    let mut x = if l.kind == LineKind::Error { ERROR_GUTTER } else { 8.0 };
    let tw = time_col::column_width(time_mode, char_w * 0.85);
    if tw > 0.0 {
        ui.painter().text(rect.left_center() + vec2(x, 0.0), Align2::LEFT_CENTER, time_col::stamp(time_mode, now_ms, l.ts_ms), FontId::monospace(11.0), theme::pal().text_dim);
        x += tw + 8.0;
    }
    if l.kind == LineKind::ToolCall {
        let w = tool_chips::chip(ui, &l.tool, rect.left_center() + vec2(x, -8.0));
        x += w.max(TAG_W - 8.0) + 8.0;
    } else {
        ui.painter().text(rect.left_center() + vec2(x, 0.0), Align2::LEFT_CENTER, kind_tag(l), FontId::monospace(11.5), if l.kind == LineKind::User { c } else { theme::pal().text_dim });
        x += TAG_W;
    }
    let avail = (rect.width() - x - 14.0).max(40.0);
    let max_chars = (avail / char_w) as usize;
    // same text as the height calc above: plain sentence only unless expanded (wrap)
    let shown = shown_text(l, wrap, max_chars);
    let mut job = tokens_render::highlighted_job(&shown, c, if l.kind == LineKind::ToolCall && l.plain.is_empty() { mono.clone() } else { FontId::proportional(mono.size + 0.5) }, query);
    if wrap {
        job.wrap.max_width = avail;
    }
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(rect.min.x + x, fade_dy + if wrap { rect.min.y + 2.0 } else { rect.center().y - galley.size().y * 0.5 });
    ui.painter().galley(pos, galley, c);
    clicked
}

/// Creature pose of a lane: a break-paused agent sleeps (zzz) and wakes when resumed.
pub fn lane_pose(state: LogState, paused: bool, errored: bool) -> creatures::Pose {
    if paused {
        return creatures::Pose::Sleeping;
    }
    if errored {
        return creatures::Pose::Error;
    }
    match state {
        LogState::Running => creatures::Pose::Working,
        LogState::Idle => creatures::Pose::Idle,
        LogState::Finished => creatures::Pose::Done,
    }
}

#[cfg(test)]
mod override_tests {
    use crate::ui::logic::picker::Overrides;
    #[test]
    fn override_file_roundtrip_and_corrupt_defaults() {
        let d = std::env::temp_dir().join(format!("bg-ov-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("o.txt");
        let mut o = Overrides::default();
        o.set("sess-1", 5);
        std::fs::write(&f, o.to_text()).unwrap();
        let back = Overrides::from_text(&std::fs::read_to_string(&f).unwrap());
        assert_eq!(back.get("sess-1"), Some(5));
        let bad = Overrides::from_text("garbage\n=\nx=999\n");
        assert_eq!(bad.get("x"), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod pose_tests {
    use super::*;
    #[test]
    fn paused_sleeps_and_resume_wakes() {
        assert_eq!(lane_pose(LogState::Running, true, false), creatures::Pose::Sleeping);
        assert_eq!(lane_pose(LogState::Running, false, false), creatures::Pose::Working);
        assert_eq!(lane_pose(LogState::Idle, true, false), creatures::Pose::Sleeping);
        assert_eq!(lane_pose(LogState::Finished, false, false), creatures::Pose::Done);
    }
}

/// Lane control: shared ghost button (focus ring drawn by the helper), named for screen readers.
/// `on` marks the active state in the label, since the ghost style has no accent colour.
fn small_btn(ui: &mut Ui, text: &str, on: bool) -> egui::Response {
    let shown = if on { format!("{text} ●") } else { text.to_string() };
    let resp = crate::ui::widgets::ghost_button(ui, &shown).on_hover_text(text);
    let name = lane_a11y::accessible_name(text).unwrap_or(text).to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name.clone()));
    resp
}

/// Progress shown under a lane header: ralp checklist ticks (with the newest ledger step) win over
/// the session scanner's progress; `None` when the session has no countable steps.
pub fn lane_progress(ralp: Option<&crate::logs::RalpFooter>, scanned: Option<(u32, u32)>) -> Option<(u32, u32, String)> {
    if let Some(f) = ralp.filter(|f| f.total > 0) {
        let step = f.ledger_tail.last().map(|l| one_line(l, 160)).unwrap_or_default();
        return Some((f.done.min(f.total), f.total, step));
    }
    scanned.filter(|(_, t)| *t > 0).map(|(d, t)| (d.min(t), t, String::new()))
}

/// Header line text for a progress row, e.g. "3 of 11 steps (27%)".
pub fn progress_text(done: u32, total: u32) -> String {
    let pct = (done * 100).checked_div(total).unwrap_or(0);
    format!("{done} of {total} steps ({pct}%)")
}

fn progress_row(ui: &mut Ui, p: &(u32, u32, String), accent: Color32) {
    let (done, total, step) = p;
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(progress_text(*done, *total)).size(13.0).strong().color(accent_text(accent, theme::is_light())));
        let w = (ui.available_width() - 4.0).max(60.0);
        ui.add_sized([w, 10.0], |ui: &mut Ui| crate::ui::widgets::progress_bar(ui, *done as f32 / (*total).max(1) as f32, accent, 10.0));
    });
    if !step.is_empty() {
        ui.add(egui::Label::new(RichText::new(format!("Latest: {step}")).size(12.5).color(theme::pal().text_dim)).truncate()).on_hover_text("Newest ledger line of this session's run");
    }
    ui.add_space(2.0);
}

#[allow(clippy::too_many_arguments)]
fn lane(ui: &mut Ui, log: &SessionLog, ui_state: &mut LaneUi, creature: usize, now_ms: i64, query: &str, font: f32, paused: bool, selected: bool, opacity: f32, unseen: u64, progress: Option<&(u32, u32, String)>) {
    ui.multiply_opacity(opacity);
    let bp = crate::ui::logic::breakpoint::parts(crate::ui::logic::breakpoint::classify(ui.available_width()));
    let view_time = ui_state.time_mode;
    let (dense, wrap) = (ui_state.dense, ui_state.wrap);
    let id = log.id.clone();
    let accent = creatures::ACCENTS[creature % creatures::COUNT];
    let sc = state_color(log.state);
    let collapsed = ui_state.layout.is_collapsed(&id);
    let solo = ui_state.layout.solo.as_deref() == Some(id.as_str());
    let mut toggle_c = false;
    let mut toggle_s = false;
    let mut mark_seen = false;
    let mut toggle_err = false;
    let view = ui_state.views.entry(id.clone()).or_default();
    // palette-aware frame here, not card_elev::elevated_frame (its fill/stroke are dark-only)
    // selection is drawn as a focus ring below, so the frame keeps its plain border
    let r = crate::ui::widgets::lane_frame(false).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let lane_top = ui.cursor().min.y;
        ui.horizontal(|ui| {
            let (cr, cr_resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
            let pop_id = ui.make_persistent_id(("creature_pick", &id));
            static PICK_DEMO: std::sync::Once = std::sync::Once::new();
            if crate::sys::is_mock() && std::env::var_os("BREAKGUARD_PICKER_DEMO").is_some() {
                if log.name.starts_with("Docs") { PICK_DEMO.call_once(|| ui.memory_mut(|m| m.open_popup(pop_id))); }
            }
            if cr_resp.on_hover_text("Click to choose this session's creature").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            let cr_resp = ui.interact(cr, pop_id.with("w"), Sense::hover());
            egui::popup::popup_below_widget(ui, pop_id, &cr_resp, egui::popup::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                ui.set_min_width(240.0);
                if let Some(i) = crate::ui::parts::lane_picker_ui::creature_picker(ui, creature) {
                    ui_state.overrides.set(&id, i);
                    save_overrides(&ui_state.overrides);
                    ui.memory_mut(|m| m.close_popup());
                }
                if ui_state.overrides.get(&id).is_some() && small_btn(ui, "reset to automatic", false).clicked() {
                    ui_state.overrides.clear(&id);
                    save_overrides(&ui_state.overrides);
                    ui.memory_mut(|m| m.close_popup());
                }
            });
            let pose = lane_pose(log.state, paused, unseen > 0);
            // breathing / busy bounce / done hop need frames; 8 fps (same as the pulse dot) keeps the
            // Live tab cheap with many lanes: every frame re-lays out every visible lane
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(125));
            creatures::draw(ui.painter(), creature, cr.center(), 28.0, pose, ui.input(|i| i.time));
            let (dot, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            crate::ui::parts::pulse_cursor::pulse_dot(ui, dot.center(), sc, log.state == LogState::Running);
            ui.label(RichText::new(&log.name).font(theme::num(16.0)).color(accent_text(accent, theme::is_light())));
            ui.label(RichText::new(creatures::NAMES[creature % creatures::COUNT]).size(11.0).color(theme::pal().text_dim));
            if bp.show_state_text {
                if paused { ui.label(RichText::new("paused").size(12.0).color(if theme::is_light() { theme::LIGHT_WARN } else { theme::WARN })); } else {
                ui.label(RichText::new(state_label(log.state)).size(12.0).color(sc)); }
            }
            let up = if log.started_ms > 0 { crate::sessions::elapsed_text((now_ms - log.started_ms) / 1000) } else { String::new() };
            if !up.is_empty() && bp.show_uptime {
                ui.label(RichText::new(format!("up {up}")).size(12.0).color(theme::pal().text_dim));
            }
            let (cur, _) = ui.allocate_exact_size(vec2(8.0, 16.0), Sense::hover());
            crate::ui::parts::pulse_cursor::blink_cursor(ui, cur.min, 14.0, accent, log.state == LogState::Running);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Collapse, if collapsed { "expand" } else { "collapse" }).clicked() {
                    toggle_c = true;
                }
                if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Solo, if solo { "solo (on)" } else { "solo" }).clicked() {
                    toggle_s = true;
                }
                if unseen > 0 {
                    let (b, b_resp) = ui.allocate_exact_size(vec2(18.0, 16.0), Sense::click());
                    error_badge::badge(ui.painter(), b.center(), unseen.min(u32::MAX as u64) as u32);
                    b_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Mark lane errors as seen"));
                    if b_resp.on_hover_text("Click to mark errors as seen").clicked() {
                        mark_seen = true;
                        toggle_err = true;
                    }
                }
            });
        });
        {
            let all: Vec<&LogLine> = log.iter().collect();
            let owned: Vec<LogLine> = all.iter().rev().take(60).rev().map(|l| (*l).clone()).collect();
            let rn = crate::ui::logic::rightnow::right_now(log.state, &owned, now_ms);
            ui.add(egui::Label::new(RichText::new(rn).size(14.0).strong().color(accent_text(accent, theme::is_light()))).truncate());
            let nd = crate::ui::logic::feed::now_doing(&owned);
            if !nd.is_empty() {
                ui.add(egui::Label::new(RichText::new(nd).size(12.0).italics().color(theme::pal().text_dim)).truncate());
            }
        }
        if let Some(p) = progress {
            progress_row(ui, p, accent);
        }
        if collapsed {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if bp.show_counters {
                ui.label(RichText::new(format!("{} lines  {} tools  {} errors",log.total_lines, log.tool_calls, log.errors)).size(12.0).color(theme::pal().text_dim));
                let times: Vec<i64> = log.iter().map(|l| l.ts_ms).collect();
                let last = times.iter().rev().find(|t| **t > 0).copied().unwrap_or(0);
                ui.label(RichText::new(crate::ui::parts::lane_header_stats::last_activity_text(now_ms, last)).size(12.0).color(theme::pal().text_dim));
                if bp.show_sparkline {
                    crate::ui::parts::sparkline::sparkline(ui, vec2(70.0, 18.0), &times, now_ms, accent);
                }
            }
            let frozen = view.freeze.is_frozen();
            if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Freeze, if frozen { "unfreeze" } else { "freeze" }).clicked() {
                if frozen { view.freeze.unfreeze() } else { view.freeze.freeze(log.total_lines) }
            }
            if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Clear, "clear").clicked() {
                view.cleared.clear(log.total_lines);
            }
            if frozen {
                ui.label(RichText::new(format!("{} new hidden", view.freeze.hidden_new(log.total_lines))).size(11.0).color(if theme::is_light() { theme::LIGHT_WARN } else { theme::WARN }));
            }
            if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Copy, "copy").clicked() {
                let ls: Vec<crate::ui::logic::copy::L> = log.iter().map(|l| crate::ui::logic::copy::L { ts_ms: l.ts_ms, tag: kind_tag(l), text: row_text(l).to_string() }).collect();
                let n = ls.len();
                ui.ctx().copy_text(crate::ui::logic::copy::copy_payload(&ls, true, None));
                view.note = format!("copied {n} lines");
            }
            if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Export, "export").clicked() {
                let lines: Vec<String> = log.iter().map(|l| format!("{} {:<8} {}", crate::ui::logic::clock::absolute(l.ts_ms), kind_tag(l), row_text(l))).collect();
                view.note = match crate::ui::logic::export::write_export(&crate::data_dir().join("exports"), &log.name, &lines) {
                    Ok(p) => format!("exported {} lines to {}", lines.len(), p.display()),
                    Err(e) => format!("export failed: {e}"),
                };
            }
            if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::CopyId, "copy id").clicked() {
                if let crate::ui::logic::actions::Action::CopyText(t) = crate::ui::logic::actions::copy_id_action(&log.id) {
                    ui.ctx().copy_text(t.clone());
                    view.note = format!("copied id {t}");
                }
            }
            if let crate::ui::logic::actions::Action::OpenFolder(p) = crate::ui::logic::actions::open_folder_action(&log.cwd) {
                if crate::ui::widgets::icon_button(ui, crate::ui::icons::Icon::Folder, "open folder").clicked() {
                    view.note = crate::ui::parts::lane_actions_ui::run(&crate::ui::parts::lane_actions_ui::LaneAction::OpenFolder(p), std::env::var("BREAKGUARD_DDC_MOCK").is_ok());
                }
            }
            if !view.note.is_empty() {
                ui.label(RichText::new(&view.note).size(11.0).color(if theme::is_light() { theme::LIGHT_OK } else { theme::OK }));
            }
        });
        let mono = FontId::monospace(font);
        let sp = crate::ui::logic::spacing::Prefs { density: if dense { crate::ui::logic::spacing::Density::Dense } else { crate::ui::logic::spacing::Density::Comfortable }, wrap, time_mode: 0, font };
        let rh = crate::ui::logic::spacing::row_height(&sp).max(font + 2.0).max(min_row_h(font, dense));
        let char_w = ui.fonts(|f| f.glyph_width(&mono, 'm'));
        // the output area takes exactly what is left of the lane height after the header, so the lane
        // never overflows its slot and the stretch grip stays visible
        let used = (ui.cursor().min.y - lane_top).max(0.0);
        let h = (crate::ui::parts::lane_scale_perf::clamp_lane_h(view.height) - used - GRIP_H - 34.0).max(rh * 3.0);
        let held = log.len();
        let end = view.freeze.visible_count(log.total_lines, held, log.dropped).min(held);
        let start = held.saturating_sub(view.cleared.visible_count(log.total_lines, held, log.dropped)).min(end);
        let lines: Vec<&LogLine> = log.iter().collect();
        let idx: Vec<usize> = (start..end).filter(|&i| query.trim().is_empty() || lines.get(i).map(|l| filter::matches(&l.text, query)).unwrap_or(false)).collect();
        let n = idx.len();
        let area = egui::ScrollArea::vertical().id_salt(scroll_salt(&id)).max_height(h).min_scrolled_height(h).auto_shrink([false, false]).stick_to_bottom(view.follow.follow);
        let wrap = wrap || view.expanded;
        let burst = log.iter().rev().take(24).filter(|l| l.ts_ms > 0 && now_ms - l.ts_ms < 450).count() > 6;
        let mut row_clicked = false;
        let transcript = std::env::var_os("BREAKGUARD_RAW_FEED").is_none();
        let out = if transcript {
            let owned: Vec<LogLine> = (start..end).filter_map(|i| lines.get(i).map(|l| (*l).clone())).filter(|l| query.trim().is_empty() || filter::matches(&l.text, query)).collect();
            let mut items = crate::ui::logic::feed::build(&owned);
            if view.errors_only {
                items = crate::ui::logic::feed::errors_only(&items);
            }
            let last_ts = owned.iter().rev().map(|l| l.ts_ms).find(|t| *t > 0).unwrap_or(0);
            if let Some(t) = crate::ui::logic::feed::thinking_row(log.state == LogState::Running, now_ms, last_ts) {
                items.push(t);
            }
            let open = &mut view.groups_open;
            // screenshot/QA hook: BREAKGUARD_GROUPS_OPEN opens every grouped row (a scripted click cannot)
            if std::env::var_os("BREAKGUARD_GROUPS_OPEN").is_some() {
                open.extend(0..items.len());
            }
            area.show(ui, |ui| {
                crate::ui::parts::feed_view::show(ui, &items, open, font, accent, now_ms, view_time);
            })
        } else if wrap {
            let from = n.saturating_sub(200);
            area.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for k in from..n {
                    if let Some(l) = lines.get(idx[k]).copied() {
                        row_clicked |= draw_row(ui, l, &mono, char_w, rh, now_ms, view_time, query, true, rh, burst);
                    }
                }
            })
        } else {
            area.show_rows(ui, rh, n, |ui, range| {
                for k in range {
                    if let Some(l) = lines.get(idx[k]).copied() {
                        row_clicked |= draw_row(ui, l, &mono, char_w, rh, now_ms, view_time, query, false, rh, burst);
                    }
                }
            })
        };
        if row_clicked {
            view.expanded = !view.expanded;
        }
        let moved = ui.rect_contains_pointer(out.inner_rect) && ui.input(|i| i.raw_scroll_delta.y != 0.0 || i.pointer.any_down());
        let max_off = (out.content_size.y - out.inner_rect.height()).max(0.0);
        view.follow.on_scroll(out.state.offset.y, max_off, moved);
        if n == 0 {
            ui.label(RichText::new("Waiting for output...").color(theme::pal().text_dim));
        }
        if view.follow.show_jump_button() {
            let b = jump_rect(out.inner_rect);
            if ui.allocate_new_ui(egui::UiBuilder::new().max_rect(b), |ui| crate::ui::widgets::ghost_button(ui, "Jump to live")).inner.clicked() {
                view.follow.jump_to_live();
            }
        }
        if view.follow.follow {
            view.seen_total = log.total_lines;
        } else if let Some(t) = crate::ui::logic::feed::new_lines_pill(true, log.total_lines.saturating_sub(view.seen_total) as usize) {
            let b = Rect::from_min_size(egui::pos2(out.inner_rect.center().x - 50.0, out.inner_rect.max.y - 58.0), vec2(100.0, 22.0));
            ui.painter().rect_filled(b, 11.0, accent.gamma_multiply(0.85));
            ui.painter().text(b.center(), Align2::CENTER_CENTER, t, FontId::proportional(12.0), Color32::BLACK);
        }
        let (grip, resp) = ui.allocate_exact_size(vec2(ui.available_width(), GRIP_H), Sense::click_and_drag());
        let hot = resp.hovered() || resp.dragged();
        if hot {
            ui.painter().rect_filled(grip, 4.0, accent.gamma_multiply(0.16));
        }
        // always-visible pill handle: a stretchable lane must look stretchable
        let pill = Rect::from_center_size(grip.center(), vec2(if hot { 120.0 } else { 72.0 }, 5.0));
        ui.painter().rect_filled(pill, 2.5, if hot { accent } else { theme::pal().text_dim.gamma_multiply(0.7) });
        let resp = resp.on_hover_text("Drag up or down to show fewer or more lines. Double-click to reset.");
        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        if resp.double_clicked() {
            view.height = LANE_H;
        } else if resp.dragged() {
            view.height = height::drag_height(view.height, resp.drag_delta().y);
        }
    });
    let rect = r.response.rect;
    if selected {
        let [fr, fg, fb] = lane_a11y::FOCUS_RING_RGB;
        ui.painter().rect_stroke(rect, theme::rad(theme::R_M), Stroke::new(lane_a11y::FOCUS_RING_WIDTH, Color32::from_rgb(fr, fg, fb)), StrokeKind::Outside);
    }
    if mark_seen {
        ui_state.badges.mark_seen(&id, log.errors);
    }
    if toggle_err {
        let v = ui_state.views.entry(id.clone()).or_default();
        v.errors_only = !v.errors_only;
    }
    ui_state.views.entry(id.clone()).or_default().last_h = rect.height();
    ui.painter().rect_filled(Rect::from_min_max(rect.min + vec2(0.0, 6.0), egui::pos2(rect.min.x + 4.0, rect.max.y - 6.0)), 2.0, accent);
    if toggle_c {
        ui_state.layout.toggle_collapse(&id);
    }
    if toggle_s {
        ui_state.layout.toggle_solo(&id);
    }
}

pub fn show(app: &mut App, ui: &mut Ui, finished_tab: bool) {
    // screenshot demo: BREAKGUARD_PAUSE_DEMO pauses every dummy agent (mock mode only), same as the Breaks page
    static DEMO: std::sync::Once = std::sync::Once::new();
    if crate::sys::is_mock() && std::env::var_os("BREAKGUARD_PAUSE_DEMO").is_some() {
        app.refresh_agents(false);
        if !app.agents.is_empty() {
            DEMO.call_once(|| {
                let all = app.agents.clone();
                app.pauser.pause(&all);
            });
        }
    }
    let hub = app.log_hub.clone();
    let Ok(h) = hub.lock() else { return };
    if h.store.logs.is_empty() {
        crate::ui::parts::empty_state::empty_lanes(ui);
        return;
    }
    let now_ms = crate::sessions::now_ms();
    let ids: Vec<String> = h.store.logs.iter().map(|l| l.id.clone()).collect();
    let mut crs = creatures::assign(&ids);
    for (i, id) in ids.iter().enumerate() {
        if let Some(k) = app.lanes.overrides.get(id) {
            crs[i] = k;
        }
    }
    app.lanes.layout.prune(&ids);
    // Live = process still alive; Finished = ended within the last few hours. An ended session leaves Live at once.
    let in_tab = |l: &crate::logs::SessionLog| {
        let mut last: Vec<LogLine> = l.iter().rev().take(2).cloned().collect();
        last.reverse();
        let working = crate::ui::logic::rightnow::is_working(l.state, &last, now_ms);
        let since = h.gone_since.get(&l.id).copied().or_else(|| last.last().map(|x| x.ts_ms).filter(|t| *t > 0));
        let b = crate::ui::logic::tabs::bucket(working, since, now_ms, crate::ui::logic::tabs::FINISHED_KEEP_MS);
        b == if finished_tab { crate::ui::logic::tabs::Bucket::Finished } else { crate::ui::logic::tabs::Bucket::Live }
    };
    let mut rows: Vec<order::Row> = h.store.logs.iter().filter(|l| in_tab(l)).map(|l| order::Row {
        id: l.id.clone(),
        name: l.name.clone(),
        state: match l.state { LogState::Running => order::State::Running, LogState::Idle => order::State::Idle, LogState::Finished => order::State::Finished },
        last_ms: l.iter().last().map(|x| x.ts_ms).unwrap_or(0),
    }).collect();
    order::sort_rows(&mut rows, app.lanes.sort);
    let sorted: Vec<String> = rows.into_iter().map(|r| r.id).collect();
    if sorted.is_empty() {
        ui.add_space(theme::SP_S);
        ui.label(RichText::new(if finished_tab { "Nothing finished in the last 3 hours." } else { "No session is running right now. Finished ones are on the Finished tab." }).size(15.0).color(theme::pal().text_dim));
        return;
    }
    let visible = app.lanes.layout.visible(&sorted);
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("lane_sort").selected_text(match app.lanes.sort { SortMode::RunningFirst => "Running first", SortMode::Name => "Name", SortMode::Newest => "Newest" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut app.lanes.sort, SortMode::RunningFirst, "Running first");
            ui.selectable_value(&mut app.lanes.sort, SortMode::Name, "Name");
            ui.selectable_value(&mut app.lanes.sort, SortMode::Newest, "Newest");
        });
        ui.add(egui::TextEdit::singleline(&mut app.lanes.filter).hint_text("Filter lines").desired_width(180.0));
        ui.label(RichText::new(format!("{} lanes", sorted.len())).size(12.0).color(theme::pal().text_dim));
        let tl = ["time: off", "time: ago", "time: clock"][(app.lanes.time_mode % 3) as usize];
        if small_btn(ui, tl, false).clicked() { app.lanes.time_mode = (app.lanes.time_mode + 1) % 3; }
        if small_btn(ui, if app.lanes.wrap { "wrap: on" } else { "wrap: off" }, app.lanes.wrap).clicked() { app.lanes.wrap = !app.lanes.wrap; }
        if small_btn(ui, if app.lanes.dense { "dense" } else { "comfortable" }, false).clicked() { app.lanes.dense = !app.lanes.dense; }
        if small_btn(ui, "A-", false).clicked() { app.lanes.font = fontsize::smaller(app.lanes.font); }
        if small_btn(ui, "A+", false).clicked() { app.lanes.font = fontsize::bigger(app.lanes.font); }
    });
    ui.add_space(theme::SP_S);
    let (query, font) = (app.lanes.filter.clone(), app.lanes.font);
    let mut ui_state = std::mem::take(&mut app.lanes);
    // keyboard: J/K move selection, Space freezes the selected lane, F focuses the filter
    let text_focused = ui.ctx().wants_keyboard_input();
    let filter_id = egui::Id::new("lane_filter");
    // Space must not freeze a lane while a button (e.g. freeze/copy) owns focus: egui activates that button instead
    let button_focused = ui.ctx().memory(|m| m.focused().is_some());
    for key in lane_keys_ui::poll_keys(ui.ctx(), text_focused) {
        if key == keys::Key::Space && button_focused {
            continue;
        }
        match ui_state.nav.handle(key, visible.len(), text_focused) {
            keys::Out::FocusFilter => ui.ctx().memory_mut(|m| m.request_focus(filter_id)),
            keys::Out::ToggleFreeze(i) => {
                if let Some(id) = visible.get(i) {
                    if let Some(l) = h.store.logs.iter().find(|l| &l.id == id) {
                        let v = ui_state.views.entry(id.clone()).or_default();
                        if v.freeze.is_frozen() { v.freeze.unfreeze() } else { v.freeze.freeze(l.total_lines) }
                    }
                }
            }
            keys::Out::Blur => ui.ctx().memory_mut(|m| m.surrender_focus(filter_id)),
            keys::Out::None => {}
        }
    }
    // side by side when the page is wide enough (LANE_MIN_W per column); otherwise one lane per row.
    // Row heights come from the fixed view.height only, never from measured content, so a new line
    // in one lane cannot move the rects of the others.
    let page_w = ui.available_width();
    let cols = lane_cols(visible.len(), page_w);
    let heights: Vec<f32> = visible.iter().map(|id| crate::ui::parts::lane_scale_perf::clamp_lane_h(ui_state.views.get(id).map(|v| v.height).unwrap_or(LANE_H)).min(crate::ui::logic::height::MAX_LANE_H)).collect();
    let slots = if cols > 1 { crate::ui::logic::grid::lane_rects(cols, page_w, theme::SP_S, &heights) } else { Vec::new() };
    let mut row_origin = egui::Pos2::ZERO;
    let mut row_top_on_screen = true;
    for (vi, id) in visible.iter().enumerate() {
        if cols > 1 && vi % cols == 0 {
            if vi > 0 {
                ui.add_space(theme::SP_S);
            }
            let end = (vi + cols).min(heights.len());
            let rh = heights[vi..end].iter().fold(0.0f32, |a, &b| a.max(b));
            let (row, _) = ui.allocate_exact_size(vec2(page_w, rh), Sense::hover());
            row_origin = row.min;
            row_top_on_screen = ui.clip_rect().expand(lane_cull_margin()).intersects(row);
        }
        let Some(pos) = h.store.logs.iter().position(|l| &l.id == id) else { continue };
        let opacity = if finished_tab { 0.75 } else { 1.0 };
        let unseen = ui_state.badges.unseen(id, h.store.logs[pos].errors);
        let selected = visible.get(ui_state.nav.selected).map(|s| s == id).unwrap_or(false);
        let (last, vh) = ui_state.views.get(id).map(|v| (v.last_h, v.height)).unwrap_or((0.0, LANE_H));
        let ph = crate::ui::parts::lane_scale_perf::placeholder_h(last, ui_state.layout.is_collapsed(id), vh);
        let slot = if cols > 1 {
            let (x, _, w, sh) = slots[vi];
            Some(Rect::from_min_size(row_origin + vec2(x, 0.0), vec2(w, sh)))
        } else {
            None
        };
        let clip = ui.clip_rect();
        let on_screen = match slot {
            Some(r) => row_top_on_screen && r.intersects(clip.expand(lane_cull_margin())),
            None => crate::ui::parts::lane_scale_perf::is_visible(ui.cursor().min.y, ph, clip.min.y, clip.max.y),
        };
        if !on_screen {
            if slot.is_none() {
                ui.allocate_space(vec2(ui.available_width(), ph));
                ui.add_space(theme::SP_S);
            }
            continue;
        }
        let scanned = app.sessions_snapshot.as_ref().and_then(|s| s.sessions.iter().find(|x| &x.id == id)).and_then(|x| x.progress);
        let prog = lane_progress(h.ralp.get(id), scanned);
        let paused = app.pauser.is_paused(&h.store.logs[pos].id);
        let mut draw = |ui: &mut Ui| {
            lane(ui, &h.store.logs[pos], &mut ui_state, crs[pos], now_ms, &query, font, paused, selected, opacity, unseen, prog.as_ref());
        };
        match slot {
            Some(r) => {
                let _ = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r), |cui| {
                    cui.set_clip_rect(lane_clip(r, cui.clip_rect()));
                    draw(cui);
                });
            }
            None => {
                draw(ui);
                ui.add_space(theme::SP_S);
            }
        }
    }
    // persist layout only when it changed
    let snap = snapshot(&ui_state);
    if snap != ui_state.saved {
        let _ = lane_persist_io::save(&layout_path(), &snap);
        ui_state.saved = snap;
    }
    app.lanes = ui_state;
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
}

#[cfg(test)]
mod contrast_tests {
    use super::*;
    use crate::ui::creatures;

    fn lin(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    }
    fn lum(c: Color32) -> f64 { 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b()) }
    fn ratio(a: Color32, b: Color32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_line_kind_color_is_aa_on_lane_surface() {
        for light in [false, true] {
            let p = if light { theme::LIGHT_PAL } else { theme::DARK_PAL };
            for k in [LineKind::Assistant, LineKind::ToolResult, LineKind::User, LineKind::Error] {
                let r = ratio(kind_color_in(k, p, light), p.surface_lo);
                assert!(r >= 4.5, "light={light} {k:?} contrast {r:.2}");
            }
        }
    }

    #[test]
    fn every_state_color_is_aa_on_lane_surface() {
        for s in [LogState::Running, LogState::Idle, LogState::Finished] {
            let r = ratio(state_color(s), theme::SURFACE_LO);
            assert!(r >= 4.5, "{s:?} contrast {r:.2}");
        }
        // light palette: the colours state_color() picks in light mode
        let light = [
            (LogState::Running, theme::LIGHT_OK),
            (LogState::Idle, theme::LIGHT_WARN),
            (LogState::Finished, theme::LIGHT_PAL.text_dim),
        ];
        for (s, c) in light {
            let r = ratio(c, theme::LIGHT_PAL.surface_lo);
            assert!(r >= 4.5, "light {s:?} contrast {r:.2}");
        }
    }

    #[test]
    fn all_12_creature_accents_are_aa_as_light_text() {
        for (i, a) in creatures::ACCENTS.iter().enumerate() {
            let r = ratio(accent_text(*a, true), theme::LIGHT_PAL.surface_lo);
            assert!(r >= 4.5, "light accent {i} contrast {r:.2}");
        }
    }

    #[test]
    fn all_12_creature_accents_are_aa_on_lane_surface() {
        assert_eq!(creatures::ACCENTS.len(), 12);
        for (i, a) in creatures::ACCENTS.iter().enumerate() {
            let r = ratio(*a, theme::SURFACE_LO);
            assert!(r >= 4.5, "accent {i} ({}) contrast {r:.2}", creatures::NAMES[i]);
        }
    }
}

#[cfg(test)]
mod layout_wiring_tests {
    use super::*;

    #[test]
    fn snapshot_roundtrips_collapsed_heights_filter_sort() {
        let mut ui = LaneUi::default();
        // Default loads the user's real saved layout from disk: start from a clean slate so the test is hermetic.
        ui.views.clear();
        ui.layout.collapsed.clear();
        ui.layout.collapsed.insert("b".into());
        ui.layout.collapsed.insert("a".into());
        ui.views.entry("c".into()).or_default().height = 300.0;
        ui.views.entry("d".into()).or_default().height = LANE_H;
        ui.filter = "err".into();
        ui.sort = SortMode::Newest;
        let snap = snapshot(&ui);
        assert_eq!(snap.collapsed, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(snap.heights, vec![("c".to_string(), 300.0)]);
        assert_eq!(snap.sort, 2);
        assert_eq!(LayoutSave::from_text(&snap.to_text()), snap);
    }

    #[test]
    fn lane_progress_prefers_ralp_then_scanner() {
        use super::{lane_progress, progress_text};
        let f = crate::logs::RalpFooter { run_dir: "x".into(), done: 3, total: 11, ledger_tail: vec!["D1 | DONE".into(), "D9 | NOTE | helper".into()] };
        assert_eq!(lane_progress(Some(&f), Some((1, 2))), Some((3, 11, "D9 | NOTE | helper".into())));
        assert_eq!(lane_progress(None, Some((5, 4))), Some((4, 4, String::new())));
        assert_eq!(lane_progress(None, Some((0, 0))), None);
        assert_eq!(lane_progress(None, None), None);
        assert_eq!(progress_text(3, 11), "3 of 11 steps (27%)");
    }

    #[test]
    fn corrupt_layout_text_gives_defaults() {
        let s = LayoutSave::from_text("\u{0}garbage\n=\n\u{0}");
        assert!(s.collapsed.is_empty());
        assert!(s.heights.is_empty());
        assert_eq!(s.sort, 0);
    }
}

#[cfg(test)]
mod lane_layout_tests {
    use super::*;
    use crate::logs::{LineKind, LogLine};
    use crate::ui::logic::grid;

    fn hit(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    }

    #[test]
    fn result_row_has_no_raw_tail() {
        let l = LogLine { ts_ms: 0, kind: LineKind::ToolResult, tool: "Read".into(), text: "first line\nRAW_TAIL_SECRET".into(), plain: "Read a file.".into() };
        assert_eq!(row_plain(&l), "Read a file.");
        assert!(!row_plain(&l).contains("RAW_TAIL"));
        assert!(row_display(&l, true).contains("RAW_TAIL"), "raw text stays available for wrap/expand mode");
    }

    #[test]
    fn row_display_default_has_no_raw_tail_for_tool_rows() {
        for kind in [LineKind::ToolCall, LineKind::ToolResult] {
            let l = LogLine { ts_ms: 0, kind, tool: "Bash".into(), text: "cargo test\nRAW_TAIL_SECRET".into(), plain: "Ran a command.".into() };
            assert_eq!(row_display(&l, false), "Ran a command.");
            assert!(!row_display(&l, false).contains("RAW_TAIL"));
        }
    }

    #[test]
    fn narrow_width_stacks() {
        assert_eq!(lane_cols(2, 2.0 * LANE_MIN_W + theme::SP_S - 1.0), 1);
        assert_eq!(lane_cols(2, 2.0 * LANE_MIN_W + theme::SP_S), 2);
        assert_eq!(lane_cols(1, 5000.0), 1);
    }

    #[test]
    fn lane_paint_clipped() {
        let r = Rect::from_min_size(egui::pos2(100.0, 100.0), vec2(420.0, 300.0));
        let parent = Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(300.0, 1000.0));
        let c = lane_clip(r, parent);
        assert_eq!(c, Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(300.0, 400.0)));
    }

    #[test]
    fn lane_scroll_ids_unique() {
        let ids = ["a", "b", "a:b", "b:a", "cli-a1", "cli-a2"];
        let salts: std::collections::HashSet<String> = ids.iter().map(|i| scroll_salt(i)).collect();
        assert_eq!(salts.len(), ids.len());
    }

    #[test]
    fn resize_clamps_no_overlap() {
        for d in [-1000.0f32, -50.0, 0.0, 37.0, 500.0, 5000.0, f32::NAN] {
            let h = crate::ui::logic::height::drag_height(LANE_H, d);
            let r = grid::lane_rects(2, 1000.0, theme::SP_S, &vec![h; 4]);
            for &(_, _, _, rh) in &r {
                assert!((crate::ui::logic::height::MIN_LANE_H..=crate::ui::logic::height::MAX_LANE_H).contains(&rh), "drag {d} h {rh}");
            }
            for i in 0..r.len() {
                for j in i + 1..r.len() {
                    assert!(!hit(r[i], r[j]), "drag {d} {i} vs {j}");
                }
            }
        }
    }

    #[test]
    fn wrap_3_4_5_lanes_no_overlap() {
        for n in [3usize, 4, 5] {
            let cols = lane_cols(n, 1400.0);
            assert!(cols >= 2, "n={n} cols={cols}");
            let hs: Vec<f32> = (0..n).map(|i| LANE_H + 11.0 * i as f32).collect();
            let r = grid::lane_rects(cols, 1400.0, theme::SP_S, &hs);
            assert_eq!(r.len(), n);
            for i in 0..n {
                for j in i + 1..n {
                    assert!(!hit(r[i], r[j]), "n={n} {i} vs {j}");
                }
            }
        }
    }

    #[test]
    fn page_rects_match_lane_rects_for_1_and_8_lanes() {
        for n in [1usize, 8] {
            let w = 1300.0;
            let cols = lane_cols(n, w);
            assert!(cols >= 1 && cols <= n);
            let hs = vec![LANE_H; n];
            let r = grid::lane_rects(cols, w, theme::SP_S, &hs);
            assert_eq!(r.len(), n);
            assert_eq!(r, grid::lane_rects(cols, w, theme::SP_S, &hs));
        }
    }

    #[test]
    fn huge_persisted_height_stays_finite() {
        let hs = vec![f32::MAX.min(crate::ui::logic::height::MAX_LANE_H); 3];
        let r = grid::lane_rects(1, 900.0, theme::SP_S, &hs);
        assert!(r.iter().all(|&(x, y, w, h)| x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()));
    }
}

#[cfg(test)]
mod lane_row_tests {
    use super::*;
    use crate::logs::{LineKind, LogLine};

    fn line(kind: LineKind, tool: &str, text: &str) -> LogLine {
        LogLine { ts_ms: 0, kind, tool: tool.into(), text: text.into(), plain: String::new() }
    }

    #[test]
    fn tag_words_plain() {
        for kind in [LineKind::Assistant, LineKind::User, LineKind::ToolResult, LineKind::Error, LineKind::ToolCall] {
            let t = kind_tag(&line(kind, "mcp__server__tool", "x"));
            assert!(!t.contains("mcp__") && !t.contains('_'), "raw id in tag {t}");
            assert!(t.chars().count() <= 12, "tag too long {t}");
        }
        assert_eq!(kind_tag(&line(LineKind::Assistant, "", "")), "claude says");
        assert_eq!(kind_tag(&line(LineKind::User, "", "")), "you");
        assert_eq!(kind_tag(&line(LineKind::ToolResult, "", "")), "done");
        assert_eq!(kind_tag(&line(LineKind::Error, "", "")), "error");
        assert_eq!(kind_tag(&line(LineKind::ToolCall, "Bash", "")), "Command");
        assert_eq!(kind_tag(&line(LineKind::ToolCall, "mcp__a__b", "")), "Tool");
    }

    #[test]
    fn assistant_text_wraps_without_ellipsis() {
        let s = "The build passed on the first try and every lane test is green now, so the next step is the release check.";
        let l = line(LineKind::Assistant, "", s);
        let no_wrap = shown_text(&l, false, 200);
        assert_eq!(no_wrap, s);
        assert!(!no_wrap.contains('\u{2026}'));
        let wrapped = shown_text(&l, true, 10);
        assert_eq!(wrapped, s);
        assert!(!wrapped.contains('\u{2026}'));
        assert!(shown_text(&l, false, 40).ends_with('\u{2026}'), "cut only when too long");
    }

    #[test]
    fn row_readability_defaults() {
        assert!(wrap_default(false), "wrap on by default");
        assert!(!wrap_default(true));
        assert_eq!(DEFAULT_TIME_MODE, 1);
        assert_eq!(["time: off", "time: ago", "time: clock"][DEFAULT_TIME_MODE as usize], "time: ago");
        for dense in [false, true] {
            for font in [10.0f32, 13.0, 16.0, 22.0] {
                assert!(min_row_h(font, dense) >= font * 1.5, "font {font} dense {dense}");
            }
        }
    }

    #[test]
    fn jump_button_inside_lane() {
        for lane in [Rect::from_min_size(egui::pos2(10.0, 20.0), vec2(400.0, 300.0)), Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(50.0, 10.0)), Rect::from_min_size(egui::pos2(5.0, 5.0), vec2(120.0, 40.0))] {
            let b = jump_rect(lane);
            assert!(b.min.x >= lane.min.x && b.min.y >= lane.min.y, "{b:?} outside {lane:?}");
            assert!(b.max.x <= lane.max.x && b.max.y <= lane.max.y, "{b:?} outside {lane:?}");
        }
    }
}
