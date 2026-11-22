//! Icons drawn with the egui painter (no icon font, always crisp, always the theme colour).
use eframe::egui::{vec2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // gallery set: some glyphs are drawn but not placed yet
pub enum Icon {
    Home,
    Monitor,
    Cup,
    Calendar,
    Moon,
    Chart,
    Gear,
    List,
    Info,
    Play,
    Pause,
    Skip,
    Plus,
    Check,
    Bell,
    Power,
    Flame,
    Drop,
    Person,
    Target,
    Sessions,
    CopyId,
    Solo,
    Collapse,
    Sun,
    Freeze,
    Clear,
    Copy,
    Export,
    Folder,
}

/// All icons, for the gallery and tests.
#[cfg(test)]
pub const ALL: [Icon; 30] = [
    Icon::Home,
    Icon::Monitor,
    Icon::Cup,
    Icon::Calendar,
    Icon::Moon,
    Icon::Chart,
    Icon::Gear,
    Icon::List,
    Icon::Info,
    Icon::Play,
    Icon::Pause,
    Icon::Skip,
    Icon::Plus,
    Icon::Check,
    Icon::Bell,
    Icon::Power,
    Icon::Flame,
    Icon::Drop,
    Icon::Person,
    Icon::Target,
    Icon::Sessions,
    Icon::CopyId,
    Icon::Solo,
    Icon::Collapse,
    Icon::Sun,
    Icon::Freeze,
    Icon::Clear,
    Icon::Copy,
    Icon::Export,
    Icon::Folder,
];

fn arc(c: Pos2, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Draw `icon` centred at `c`, fitting a `size` x `size` box, in `col`.
pub fn paint(p: &Painter, icon: Icon, c: Pos2, size: f32, col: Color32) {
    let u = size / 20.0; // design grid: 20 units
    let w = (1.7 * u).max(1.2);
    let st = Stroke::new(w, col);
    let pt = |x: f32, y: f32| c + vec2(x * u, y * u);
    let line = |pts: Vec<Pos2>| {
        p.add(Shape::line(pts, st));
    };
    match icon {
        Icon::Home => {
            line(vec![pt(-8.0, 0.0), pt(0.0, -8.0), pt(8.0, 0.0)]);
            line(vec![pt(-5.5, -2.0), pt(-5.5, 7.5), pt(5.5, 7.5), pt(5.5, -2.0)]);
            line(vec![pt(-2.0, 7.5), pt(-2.0, 2.5), pt(2.0, 2.5), pt(2.0, 7.5)]);
        }
        Icon::Monitor => {
            p.rect_stroke(Rect::from_center_size(pt(0.0, -1.0), vec2(17.0 * u, 11.0 * u)), 2.0 * u, st, StrokeKind::Middle);
            line(vec![pt(0.0, 4.5), pt(0.0, 8.0)]);
            line(vec![pt(-4.0, 8.0), pt(4.0, 8.0)]);
        }
        Icon::Cup => {
            line(vec![pt(-6.0, -2.0), pt(-5.0, 6.0), pt(4.0, 6.0), pt(5.0, -2.0), pt(-6.0, -2.0)]);
            line(arc(pt(5.6, 2.0), 3.0 * u, -PI / 2.0, PI / 2.0, 10));
            line(vec![pt(-3.0, -5.0), pt(-2.0, -7.5)]);
            line(vec![pt(0.5, -5.0), pt(1.5, -7.5)]);
        }
        Icon::Calendar => {
            p.rect_stroke(Rect::from_center_size(pt(0.0, 1.0), vec2(16.0 * u, 14.0 * u)), 2.0 * u, st, StrokeKind::Middle);
            line(vec![pt(-8.0, -2.0), pt(8.0, -2.0)]);
            line(vec![pt(-4.0, -7.0), pt(-4.0, -4.0)]);
            line(vec![pt(4.0, -7.0), pt(4.0, -4.0)]);
            p.circle_filled(pt(-3.0, 3.0), 1.2 * u, col);
            p.circle_filled(pt(1.0, 3.0), 1.2 * u, col);
            p.circle_filled(pt(5.0, 3.0), 1.2 * u, col);
        }
        Icon::Moon => {
            // crescent: outer arc, then inner arc back
            // outer circle r=8 at the origin and inner circle r=6.5 at (4,-3) meet at (7.63,2.39) and (-0.15,-8.0)
            let mut pts = arc(pt(0.0, 0.0), 8.0 * u, 0.304, 4.694, 26);
            pts.extend(arc(pt(4.0, -3.0), 6.5 * u, 4.018, 0.978, 22));
            line(pts);
        }
        Icon::Chart => {
            for (i, h) in [6.0_f32, 11.0, 8.0, 14.0].iter().enumerate() {
                let x = -7.5 + i as f32 * 5.0;
                p.rect_filled(Rect::from_min_max(pt(x - 1.6, 8.0 - h), pt(x + 1.6, 8.0)), 1.2 * u, col);
            }
        }
        Icon::Gear => {
            p.circle_stroke(pt(0.0, 0.0), 3.0 * u, st);
            p.circle_stroke(pt(0.0, 0.0), 6.2 * u, st);
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                line(vec![c + d * 6.2 * u, c + d * 8.6 * u]);
            }
        }
        Icon::List => {
            for y in [-5.5_f32, 0.0, 5.5] {
                p.circle_filled(pt(-7.0, y), 1.3 * u, col);
                line(vec![pt(-3.5, y), pt(8.0, y)]);
            }
        }
        Icon::Info => {
            p.circle_stroke(pt(0.0, 0.0), 8.5 * u, st);
            p.circle_filled(pt(0.0, -4.0), 1.2 * u, col);
            line(vec![pt(0.0, -1.0), pt(0.0, 5.0)]);
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![pt(-4.5, -7.0), pt(7.0, 0.0), pt(-4.5, 7.0)], col, Stroke::NONE));
        }
        Icon::Pause => {
            p.rect_filled(Rect::from_min_max(pt(-6.0, -7.0), pt(-2.0, 7.0)), 1.2 * u, col);
            p.rect_filled(Rect::from_min_max(pt(2.0, -7.0), pt(6.0, 7.0)), 1.2 * u, col);
        }
        Icon::Skip => {
            p.add(Shape::convex_polygon(vec![pt(-7.0, -6.5), pt(3.5, 0.0), pt(-7.0, 6.5)], col, Stroke::NONE));
            p.rect_filled(Rect::from_min_max(pt(5.0, -6.5), pt(8.0, 6.5)), 1.0 * u, col);
        }
        Icon::Plus => {
            line(vec![pt(-7.0, 0.0), pt(7.0, 0.0)]);
            line(vec![pt(0.0, -7.0), pt(0.0, 7.0)]);
        }
        Icon::Check => {
            line(vec![pt(-7.0, 0.5), pt(-2.5, 5.0), pt(7.5, -5.5)]);
        }
        Icon::Bell => {
            let mut pts = arc(pt(0.0, -1.0), 6.0 * u, PI, TAU, 14);
            pts.extend([pt(6.0, 5.0), pt(7.5, 6.0), pt(-7.5, 6.0), pt(-6.0, 5.0)]);
            pts.push(pts[0]);
            line(pts);
            line(arc(pt(0.0, 6.5), 2.0 * u, 0.0, PI, 6));
        }
        Icon::Power => {
            line(arc(pt(0.0, 1.0), 7.0 * u, -PI / 2.0 + 0.7, -PI / 2.0 + TAU - 0.7, 24));
            line(vec![pt(0.0, -8.0), pt(0.0, 0.0)]);
        }
        Icon::Flame => {
            line(vec![
                pt(0.0, -9.0), pt(4.5, -2.5), pt(6.0, 2.0), pt(3.5, 7.0), pt(0.0, 8.5), pt(-3.5, 7.0), pt(-6.0, 2.0),
                pt(-4.0, -2.0), pt(-1.5, 0.0), pt(0.0, -9.0),
            ]);
        }
        Icon::Drop => {
            let mut pts = vec![pt(0.0, -8.5)];
            pts.extend(arc(pt(0.0, 3.0), 5.5 * u, -PI / 2.0 + 0.9, -PI / 2.0 + TAU - 0.9, 18));
            pts.push(pt(0.0, -8.5));
            line(pts);
        }
        Icon::Person => {
            p.circle_stroke(pt(0.0, -4.0), 3.5 * u, st);
            line(arc(pt(0.0, 9.0), 7.5 * u, PI + 0.35, TAU - 0.35, 12));
        }
        Icon::Target => {
            p.circle_stroke(pt(0.0, 0.0), 8.0 * u, st);
            p.circle_stroke(pt(0.0, 0.0), 4.2 * u, st);
            p.circle_filled(pt(0.0, 0.0), 1.2 * u, col);
        }
        Icon::Sessions => {
            p.rect_stroke(Rect::from_min_max(pt(-8.0, -6.0), pt(4.0, 3.0)), 2.0 * u, st, StrokeKind::Middle);
            line(vec![pt(-4.0, 6.0), pt(8.0, 6.0), pt(8.0, -2.0)]);
            line(vec![pt(-5.0, -2.0), pt(-2.0, -2.0)]);
            line(vec![pt(-5.0, 0.5), pt(0.0, 0.5)]);
        }
        Icon::CopyId => {
            // Two sheets plus a tag line: copy with an identifier.
            p.rect_stroke(Rect::from_min_max(pt(-6.0, -5.0), pt(3.0, 4.0)), 1.5 * u, st, StrokeKind::Middle);
            line(vec![pt(-3.0, -8.0), pt(7.0, -8.0), pt(7.0, 2.0)]);
            line(vec![pt(-2.5, 0.5), pt(0.5, 0.5)]);
        }
        Icon::Solo => {
            // Single filled dot with a ring: this one only.
            p.circle_stroke(pt(0.0, 0.0), 7.0 * u, st);
            p.circle_filled(pt(0.0, 0.0), 2.6 * u, col);
        }
        Icon::Collapse => {
            // Chevron pair pointing toward the middle.
            line(vec![pt(-7.0, -6.0), pt(-2.0, -1.0), pt(-7.0, 4.0)]);
            line(vec![pt(7.0, -6.0), pt(2.0, -1.0), pt(7.0, 4.0)]);
        }
        Icon::Sun => {
            p.circle_stroke(pt(0.0, 0.0), 4.0 * u, st);
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                line(vec![c + d * 6.5 * u, c + d * 8.8 * u]);
            }
        }
        Icon::Freeze => {
            // Snowflake: three crossing strokes at 60 degree steps.
            for k in 0..3 {
                let a = k as f32 * PI / 3.0;
                let d = vec2(a.cos(), a.sin()) * 7.0 * u;
                line(vec![c - d, c + d]);
            }
        }
        Icon::Clear => {
            // Eraser: slanted block over a baseline.
            line(vec![pt(-6.0, 3.0), pt(1.0, -4.0), pt(8.0, 2.0), pt(3.0, 7.0), pt(-6.0, 3.0)]);
            line(vec![pt(-4.0, 8.0), pt(8.0, 8.0)]);
        }
        Icon::Copy => {
            p.rect_stroke(Rect::from_min_max(pt(-2.8, -2.8), pt(7.2, 7.2)), 1.5 * u, st, StrokeKind::Middle);
            line(vec![pt(-7.2, 2.8), pt(-7.2, -7.2), pt(2.8, -7.2)]);
        }
        Icon::Export => {
            // Tray with an arrow pointing up out of it.
            line(vec![pt(-6.8, 1.2), pt(-6.8, 6.8), pt(6.8, 6.8), pt(6.8, 1.2)]);
            line(vec![pt(0.0, 3.2), pt(0.0, -7.2)]);
            line(vec![pt(-4.0, -3.2), pt(0.0, -7.2), pt(4.0, -3.2)]);
        }
        Icon::Folder => {
            line(vec![pt(-7.6, -4.4), pt(-7.6, 6.8), pt(7.6, 6.8), pt(7.6, -2.8), pt(-0.8, -2.8), pt(-2.4, -4.4), pt(-7.6, -4.4)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_paints_something() {
        let ctx = eframe::egui::Context::default();
        for icon in ALL {
            // each icon adds at least one shape on top of the empty panel background
            let empty = ctx.run(Default::default(), |ctx| {
                eframe::egui::CentralPanel::default().show(ctx, |_ui| {});
            });
            let out = ctx.run(Default::default(), |ctx| {
                eframe::egui::CentralPanel::default().show(ctx, |ui| {
                    paint(ui.painter(), icon, eframe::egui::pos2(20.0, 20.0), 20.0, Color32::WHITE);
                });
            });
            assert!(out.shapes.len() > empty.shapes.len(), "{icon:?} drew nothing");
        }
        assert_eq!(ALL.len(), 30);
    }

    /// Every variant named in ui sources (the bare path form) must be in `ALL`.
    /// Qualified `CursorIcon::...` uses are not ours and are skipped.
    #[test]
    fn icon_names_used_in_pages_exist() {
        fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).expect("read ui dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(std::fs::read_to_string(&p).expect("read source"));
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
        let mut sources = Vec::new();
        walk(&root, &mut sources);
        let names: Vec<String> = ALL.iter().map(|i| format!("{i:?}")).collect();
        let mut checked = 0usize;
        for src in &sources {
            for (idx, _) in src.match_indices("Icon::") {
                let before = src[..idx].chars().last();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue; // e.g. CursorIcon::PointingHand
                }
                let name: String = src[idx + 6..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if name.is_empty() || name == "paint" || name == "ALL" {
                    continue;
                }
                checked += 1;
                assert!(names.contains(&name), "Icon::{name} used in ui source but missing from icons::ALL");
            }
        }
        assert!(checked > 20, "scan found too few uses: {checked}");
    }
}
