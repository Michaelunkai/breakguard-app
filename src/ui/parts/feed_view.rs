//! Transcript-style renderer for the Live feed (model in ui::logic::feed).
use crate::ui::logic::feed::{self, ChipKind, Font, Item, SpanKind};
use crate::ui::theme;
use eframe::egui::{self, text::LayoutJob, Color32, FontId, Label, RichText, TextFormat, Ui};
use std::collections::HashSet;

fn danger() -> Color32 {
    if theme::is_light() { theme::LIGHT_DANGER } else { theme::DANGER }
}

fn font_of(f: Font, size: f32) -> FontId {
    match f {
        Font::Mono => FontId::monospace(size - 1.0),
        Font::Proportional => FontId::proportional(size),
    }
}

/// Prose as one wrapped job: proportional text, mono only for code and paths.
pub fn prose_job(text: &str, size: f32, color: Color32, accent: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for s in feed::md_spans(text) {
        let (c, bold) = match s.kind {
            SpanKind::Plain => (color, false),
            SpanKind::Bold => (theme::pal().text, true),
            SpanKind::Code => (accent, false),
            SpanKind::Path => (accent, false),
        };
        let mut fmt = TextFormat::simple(font_of(feed::font_for(s.kind), size), c);
        if bold {
            fmt.underline = egui::Stroke::NONE;
        }
        if s.kind == SpanKind::Code {
            fmt.background = theme::pal().border.gamma_multiply(0.45);
        }
        job.append(&s.text, 0.0, fmt);
    }
    job
}

fn chip_color(k: ChipKind) -> Color32 {
    match k {
        ChipKind::Ticked => theme::OK,
        ChipKind::Added => Color32::from_rgb(96, 165, 250),
        ChipKind::JobStarted => theme::WARN,
        ChipKind::JobEnded => Color32::from_rgb(167, 139, 250),
        ChipKind::Forecast => Color32::from_rgb(45, 212, 191),
    }
}

fn time_cell(ui: &mut Ui, stamp: &str) {
    ui.add_sized([44.0, 14.0], Label::new(RichText::new(stamp).size(10.0).color(theme::pal().text_dim)));
}

/// Draws all items. `open` = expanded group indices. Returns nothing; clicks mutate `open`.
pub fn show(ui: &mut Ui, items: &[Item], open: &mut HashSet<usize>, font: f32, accent: Color32, now_ms: i64, time_mode: u8) {
    let dim = theme::pal().text_dim;
    let text = theme::pal().text;
    let size = font + 1.0;
    let stamps: Vec<String> = items
        .iter()
        .map(|i| match i {
            Item::User { ts_ms, .. } | Item::Prose { ts_ms, .. } | Item::Group { ts_ms, .. } | Item::Chip { ts_ms, .. } | Item::Error { ts_ms, .. } => crate::ui::parts::time_col::stamp(if time_mode == 0 { 2 } else { time_mode }, now_ms, *ts_ms),
            _ => String::new(),
        })
        .collect();
    let stamps = feed::collapse_times(&stamps);
    ui.spacing_mut().item_spacing.y = 5.0;
    for (i, item) in items.iter().enumerate() {
        match item {
            Item::User { text: t, .. } => {
                egui::Frame::new().fill(accent.gamma_multiply(0.18)).stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.5))).corner_radius(8.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(Label::new(RichText::new(t).size(size).color(text)).wrap());
                });
            }
            Item::Prose { text: t, .. } => {
                ui.horizontal_top(|ui| {
                    time_cell(ui, &stamps[i]);
                    ui.add(Label::new(prose_job(t, size, text, accent)).wrap());
                });
            }
            Item::Group { calls, .. } => {
                let is_open = open.contains(&i);
                let any_failed = calls.iter().any(|c| c.failed);
                let resp = ui
                    .horizontal(|ui| {
                        time_cell(ui, &stamps[i]);
                        let chev = if is_open { "\u{25be}" } else { "\u{25b8}" };
                        let col = if any_failed { danger() } else { dim };
                        ui.add(Label::new(RichText::new(format!("{chev} {}", feed::group_label(calls))).size(size - 1.0).color(col)).sense(egui::Sense::click()))
                    })
                    .inner
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    if is_open { open.remove(&i); } else { open.insert(i); }
                }
                if is_open {
                    ui.indent(("grp", i), |ui| {
                        for c in calls {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(&c.desc).size(size - 1.0).color(text));
                            });
                            if c.failed {
                                // plain sentence: "It failed: <short reason>" (paths cut to file names)
                                let why = crate::logs::plain_result(&c.out.join("\n"), true);
                                ui.add(Label::new(RichText::new(why).size(size - 1.0).strong().color(danger())).wrap());
                            }
                            if !c.cmd.is_empty() && c.cmd != c.desc {
                                ui.add(Label::new(RichText::new(&c.cmd).font(FontId::monospace(size - 2.0)).color(dim)).wrap());
                            }
                            for o in &c.out {
                                ui.add(Label::new(RichText::new(o).font(FontId::monospace(size - 3.0)).color(dim)).wrap());
                            }
                        }
                    });
                }
            }
            Item::Chip { kind, text: t, .. } => {
                ui.horizontal(|ui| {
                    time_cell(ui, &stamps[i]);
                    let c = chip_color(*kind);
                    egui::Frame::new().fill(c.gamma_multiply(0.2)).stroke(egui::Stroke::new(1.0, c.gamma_multiply(0.7))).corner_radius(10.0).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                        ui.label(RichText::new(t).size(size - 1.0).color(c));
                    });
                });
            }
            Item::Error { text: t, .. } => {
                ui.horizontal_top(|ui| {
                    time_cell(ui, &stamps[i]);
                    ui.add(Label::new(RichText::new(t).size(size).color(danger())).wrap());
                });
            }
            Item::Divider => {
                ui.add_space(8.0);
                ui.separator();
            }
            Item::Thinking { secs } => {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(400));
                ui.horizontal(|ui| {
                    time_cell(ui, "");
                    ui.label(RichText::new(feed::thinking_text(*secs, now_ms)).size(size - 1.0).italics().color(dim));
                });
            }
        }
    }
}
