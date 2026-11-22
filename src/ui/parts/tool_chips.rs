//! API: pub fn chip(ui: &mut Ui, tool: &str, at: Pos2) -> f32  (paints chip with top-left at `at`, returns width)
//! API: pub fn chip_color(tool: &str) -> Color32  (deterministic chip fill per tool; unknown tools get the fallback fill)
//! API: pub fn chip_label_color(fill: Color32) -> Color32  (TEXT or ON_FILL, whichever has the higher contrast on `fill`)
//! API: pub fn chip_width(ui: &Ui, tool: &str) -> f32  (width `chip` will use, for layout without painting)
#![allow(dead_code)]

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2,
};

use crate::ui::logic::contrast;
use crate::ui::theme;

const CHIP_H: f32 = 20.0;
const PAD_X: f32 = 6.0;
const ICON: f32 = 12.0;
const GAP: f32 = 4.0;
const FONT_SIZE: f32 = 11.0;

const BASH_FILL: Color32 = Color32::from_rgb(251, 191, 36);
const READ_FILL: Color32 = Color32::from_rgb(96, 165, 250);
const EDIT_FILL: Color32 = Color32::from_rgb(192, 132, 252);
const WRITE_FILL: Color32 = Color32::from_rgb(74, 222, 128);
const GREP_FILL: Color32 = Color32::from_rgb(248, 113, 113);
const OTHER_FILL: Color32 = Color32::from_rgb(148, 163, 184);

fn rgb(c: Color32) -> (u8, u8, u8) {
    (c.r(), c.g(), c.b())
}

/// Fill colour for a tool name. Deterministic; unknown tools use the fallback.
pub fn chip_color(tool: &str) -> Color32 {
    match tool {
        "Bash" => BASH_FILL,
        "Read" => READ_FILL,
        "Edit" => EDIT_FILL,
        "Write" => WRITE_FILL,
        "Grep" => GREP_FILL,
        _ => OTHER_FILL,
    }
}

/// Label colour with the best WCAG contrast on `fill` (TEXT or ON_FILL).
pub fn chip_label_color(fill: Color32) -> Color32 {
    let light = contrast::ratio(rgb(theme::TEXT), rgb(fill));
    let dark = contrast::ratio(rgb(theme::ON_FILL), rgb(fill));
    if light >= dark {
        theme::TEXT
    } else {
        theme::ON_FILL
    }
}

/// Plain-word label shown on the chip for a tool. Unknown and MCP tools show `Tool`.
pub fn plain_label(tool: &str) -> &'static str {
    if tool.starts_with("mcp__") {
        return "Tool";
    }
    match tool {
        "Bash" | "PowerShell" => "Command",
        "Read" => "Read",
        "Edit" | "MultiEdit" | "NotebookEdit" => "Edit",
        "Write" => "Create",
        "Grep" | "Glob" => "Search",
        "Agent" | "Task" => "Helper",
        "WebFetch" | "WebSearch" => "Web",
        _ => "Tool",
    }
}

#[cfg(test)]
mod plain_label_tests {
    use super::plain_label;

    #[test]
    fn maps_known_tools_to_plain_words() {
        assert_eq!(plain_label("Bash"), "Command");
        assert_eq!(plain_label("PowerShell"), "Command");
        assert_eq!(plain_label("Read"), "Read");
        assert_eq!(plain_label("Edit"), "Edit");
        assert_eq!(plain_label("MultiEdit"), "Edit");
        assert_eq!(plain_label("NotebookEdit"), "Edit");
        assert_eq!(plain_label("Write"), "Create");
        assert_eq!(plain_label("Grep"), "Search");
        assert_eq!(plain_label("Glob"), "Search");
        assert_eq!(plain_label("Agent"), "Helper");
        assert_eq!(plain_label("Task"), "Helper");
        assert_eq!(plain_label("WebFetch"), "Web");
        assert_eq!(plain_label("WebSearch"), "Web");
    }

    #[test]
    fn mcp_and_unknown_tools_show_tool() {
        assert_eq!(plain_label("mcp__ccd_session_mgmt__list"), "Tool");
        assert_eq!(plain_label("mcp__x__a_really_long_tool_name_here"), "Tool");
        assert_eq!(plain_label("NoSuchTool"), "Tool");
        assert_eq!(plain_label(""), "Tool");
    }
}

fn text_width(painter: &Painter, text: &str, color: Color32) -> f32 {
    painter
        .layout_no_wrap(text.to_owned(), FontId::proportional(FONT_SIZE), color)
        .size()
        .x
}

/// Total chip width for `tool`, using the same measurement as `chip`.
pub fn chip_width(ui: &Ui, tool: &str) -> f32 {
    let w = text_width(ui.painter(), plain_label(tool), theme::TEXT);
    PAD_X + ICON + GAP + w + PAD_X
}

/// Paints the icon for `tool` inside the square `r` (r.height() == ICON).
fn paint_icon(p: &Painter, tool: &str, r: Rect, col: Color32) {
    let stroke = Stroke::new(1.5_f32, col);
    let c = r.center();
    let h = r.height() * 0.5;
    match tool {
        "Bash" => {
            // chevron ">" and underscore
            p.line_segment([c + Vec2::new(-h * 0.6, -h * 0.5), c + Vec2::new(0.0, 0.0)], stroke);
            p.line_segment([c + Vec2::new(0.0, 0.0), c + Vec2::new(-h * 0.6, h * 0.5)], stroke);
            p.line_segment([c + Vec2::new(0.2 * h, h * 0.6), c + Vec2::new(h * 0.8, h * 0.6)], stroke);
        }
        "Read" => {
            // eye: circle with pupil
            p.circle_stroke(c, h * 0.9, stroke);
            p.circle_filled(c, h * 0.3, col);
        }
        "Edit" => {
            // pencil: diagonal stroke
            p.line_segment([c + Vec2::new(-h, h), c + Vec2::new(h, -h)], Stroke::new(2.5_f32, col));
        }
        "Write" => {
            // document: outlined rect with two lines
            let doc = Rect::from_center_size(c, Vec2::new(h * 1.5, h * 1.9));
            p.rect_stroke(doc, CornerRadius::same(1), stroke, StrokeKind::Inside);
            p.line_segment([doc.left_center() + Vec2::new(2.0, -2.0), doc.right_center() + Vec2::new(-2.0, -2.0)], stroke);
            p.line_segment([doc.left_center() + Vec2::new(2.0, 2.0), doc.right_center() + Vec2::new(-2.0, 2.0)], stroke);
        }
        "Grep" => {
            // magnifier: ring plus handle
            let ring = c + Vec2::new(-h * 0.2, -h * 0.2);
            p.circle_stroke(ring, h * 0.6, stroke);
            p.line_segment([ring + Vec2::new(h * 0.45, h * 0.45), c + Vec2::new(h * 0.9, h * 0.9)], stroke);
        }
        _ => {
            // unknown: filled dot
            p.circle_filled(c, h * 0.45, col);
        }
    }
}

/// Paints a rounded tool chip with its top-left corner at `at`. Returns the width used.
pub fn chip(ui: &mut Ui, tool: &str, at: Pos2) -> f32 {
    let fill = chip_color(tool);
    let label = chip_label_color(fill);
    let painter = ui.painter().clone();

    let name = plain_label(tool);
    let text_w = text_width(&painter, name, label);
    let width = PAD_X + ICON + GAP + text_w + PAD_X;
    let rect = Rect::from_min_size(at, Vec2::new(width, CHIP_H));

    painter.rect_filled(rect, CornerRadius::same((CHIP_H * 0.5) as u8), fill);

    let icon_rect = Rect::from_min_size(
        Pos2::new(rect.left() + PAD_X, rect.center().y - ICON * 0.5),
        Vec2::splat(ICON),
    );
    paint_icon(&painter, tool, icon_rect, label);

    let text_pos = Pos2::new(icon_rect.right() + GAP, rect.center().y);
    painter.text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        name,
        FontId::proportional(FONT_SIZE),
        label,
    );

    width
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: [&str; 5] = ["Bash", "Read", "Edit", "Write", "Grep"];

    #[test]
    fn known_tools_have_distinct_colours() {
        for (i, a) in KNOWN.iter().enumerate() {
            for b in &KNOWN[i + 1..] {
                assert_ne!(chip_color(a), chip_color(b), "{a} vs {b}");
            }
        }
    }

    #[test]
    fn known_tools_differ_from_fallback() {
        let fallback = chip_color("NoSuchTool");
        for t in KNOWN {
            assert_ne!(chip_color(t), fallback, "{t} equals fallback");
        }
    }

    #[test]
    fn unknown_tool_uses_fallback_and_is_deterministic() {
        assert_eq!(chip_color("Glob"), chip_color(""));
        assert_eq!(chip_color("Glob"), chip_color("Glob"));
        assert_eq!(chip_color("Glob"), OTHER_FILL);
    }

    #[test]
    fn label_contrast_at_least_4_5_for_all_tools() {
        for t in KNOWN.iter().copied().chain(["Glob", "", "Очень-длинное-имя"]) {
            let fill = chip_color(t);
            let label = chip_label_color(fill);
            let r = contrast::ratio(rgb(label), rgb(fill));
            assert!(r >= 4.5, "{t}: contrast {r:.2} < 4.5");
        }
    }

    #[test]
    fn tool_chip_labels_are_plain_words() {
        assert_eq!(plain_label("Bash"), "Command");
        assert_eq!(plain_label("mcp__x__y"), "Tool");
        assert!(plain_label("Grep").chars().all(|c| c.is_ascii_alphabetic()));
    }
}
