use crate::ui::App;
use eframe::egui::{self, Ui};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn log_path() -> PathBuf {
    if std::env::var_os("BREAKGUARD_DATA_DIR").is_none() {
        if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
            return dir.join("breakguard.log");
        }
    }
    crate::engine::data_dir().join("breakguard.log")
}

fn read_tail(max_bytes: u64) -> String {
    let Ok(mut f) = std::fs::File::open(log_path()) else {
        return "(no log yet)".into();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len > max_bytes {
        let _ = f.seek(SeekFrom::Start(len - max_bytes));
    }
    let mut buf = Vec::new();
    let _ = f.read_to_end(&mut buf);
    let s = String::from_utf8_lossy(&buf).into_owned();
    let s = if len > max_bytes { s.split_once('\n').map(|x| x.1.to_string()).unwrap_or(s) } else { s };
    if s.trim().is_empty() {
        "(log is empty)".into()
    } else {
        s
    }
}

pub fn show(app: &mut App, ui: &mut Ui) {
    super::title(ui, "Activity log");
    let mut tab = app.log_tab;
    if crate::ui::widgets::segmented(ui, &mut tab, &["Live", "Activity", "Sessions"]) {
        app.log_tab = tab;
    }
    ui.add_space(6.0);
    match app.log_tab {
        0 => return super::lanes::show(app, ui),
        2 => return sessions_tab(app, ui),
        _ => {}
    }
    if app.log_cache.0.elapsed() > Duration::from_secs(2) {
        app.log_cache = (Instant::now(), read_tail(32 * 1024));
    }
    if crate::ui::widgets::ghost_button(ui, "Refresh").on_hover_text("Reload the log now").clicked() {
        app.log_cache.0 = Instant::now() - Duration::from_secs(60);
    }
    ui.add_space(6.0);
    if app.log_cache.1.starts_with("(no log yet)") || app.log_cache.1.starts_with("(log is empty)") {
        crate::ui::widgets::card(ui, "", |ui| {
            ui.label(egui::RichText::new("No activity yet.").size(16.0));
            ui.label(egui::RichText::new("Screen-offs, schedules and any failures will be listed here.").color(crate::ui::theme::pal().text_dim));
        });
        return;
    }
    let lines: Vec<&str> = app.log_cache.1.lines().collect();
    let start = lines.len().saturating_sub(300);
    for l in &lines[start..] {
        // Failures stand out so a DDC or sleep problem is visible at a glance.
        let color = line_color(line_kind(l), crate::ui::theme::is_light());
        ui.label(egui::RichText::new(*l).monospace().color(color));
    }
}

/// Per-session log tail: the last few activity lines of every known session.
fn sessions_tab(app: &mut App, ui: &mut Ui) {
    use crate::ui::{theme, widgets};
    let Some(snap) = app.sessions_snapshot.clone() else {
        ui.label(egui::RichText::new("Reading sessions...").color(theme::pal().text_dim));
        return;
    };
    let list: Vec<_> = snap.sessions.iter().filter(|s| !s.archived).collect();
    if list.is_empty() {
        widgets::card(ui, "", |ui| {
            ui.label(egui::RichText::new("No sessions found.").size(16.0));
        });
        return;
    }
    egui::ScrollArea::vertical().id_salt("log_sessions").show(ui, |ui| {
        for s in list {
            widgets::card(ui, &s.name, |ui| {
                if s.tail.is_empty() {
                    ui.label(egui::RichText::new(s.note.clone().unwrap_or_else(|| "No activity recorded yet.".into())).color(theme::pal().text_dim));
                }
                for l in &s.tail {
                    ui.label(egui::RichText::new(l).monospace());
                }
            });
        }
    });
}

/// Colour class of one log line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Error,
    Ok,
    Info,
}

/// True for log lines that report a failed or rejected action.
pub fn is_error_line(l: &str) -> bool {
    let l = l.to_ascii_lowercase();
    ["failed", "rejected", "could not", "not found", "-> fail"].iter().any(|k| l.contains(k))
}

pub fn line_kind(l: &str) -> LineKind {
    if is_error_line(l) {
        LineKind::Error
    } else if l.to_ascii_lowercase().starts_with("ok") || ["succeeded", "answers some", " ok"].iter().any(|k| l.to_ascii_lowercase().contains(k)) {
        LineKind::Ok
    } else {
        LineKind::Info
    }
}

/// Text colour for a line kind on the given theme. Info uses the palette text colour.
pub fn line_color(kind: LineKind, light: bool) -> egui::Color32 {
    use crate::ui::theme::{self, DANGER_HI, LIGHT_DANGER, LIGHT_OK, OK};
    match (kind, light) {
        (LineKind::Error, false) => DANGER_HI,
        (LineKind::Error, true) => LIGHT_DANGER,
        (LineKind::Ok, false) => OK,
        (LineKind::Ok, true) => LIGHT_OK,
        (LineKind::Info, l) => if l { theme::LIGHT_PAL.text } else { theme::DARK_PAL.text },
    }
}

#[cfg(test)]
mod tests {
    use super::{is_error_line, line_color, LineKind};
    use crate::ui::logic::contrast::passes_aa;
    use crate::ui::theme::{DARK_PAL, LIGHT_PAL};

    fn rgb(c: eframe::egui::Color32) -> (u8, u8, u8) {
        (c.r(), c.g(), c.b())
    }

    #[test]
    fn every_line_color_passes_aa_on_both_themes() {
        for (light, pal) in [(false, DARK_PAL), (true, LIGHT_PAL)] {
            for kind in [LineKind::Error, LineKind::Ok, LineKind::Info] {
                let fg = rgb(line_color(kind, light));
                for bg in [pal.bg, pal.surface, pal.surface_hi, pal.surface_lo] {
                    assert!(passes_aa(fg, rgb(bg)), "{kind:?} light={light} fg={fg:?} bg={:?}", rgb(bg));
                }
            }
        }
    }


    #[test]
    fn error_lines_flagged() {
        assert!(is_error_line("turn on FAILED \\\\.\\DISPLAY1: monitor still answers None"));
        assert!(is_error_line("hardware off x for 5 min: off command rejected, helper disarmed"));
        assert!(is_error_line("blackout: monitor x not found"));
        assert!(!is_error_line("turn on (timer) \\\\.\\DISPLAY1: 1 tries, monitor answers Some(1)"));
        assert!(!is_error_line("blackout ended"));
    }
}
