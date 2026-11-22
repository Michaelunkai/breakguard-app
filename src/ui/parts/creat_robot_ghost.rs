//! slice creat_robot_ghost (owned by one team agent)
//! API: pub fn draw_robot(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! API: pub fn draw_ghost(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! API: Pose = crate::ui::creatures::Pose {Idle, Working, Sleeping, Done}; s = square side in px; t = seconds (animation clock).
//! Painter primitives only, no fonts/assets. Sleeping draws three stroked "z" glyphs.
#![allow(dead_code)]
use crate::ui::creatures::Pose;
use eframe::egui::{vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

const ROBOT_BODY: Color32 = Color32::from_rgb(148, 163, 184);
const ROBOT_DARK: Color32 = Color32::from_rgb(71, 85, 105);
const ROBOT_FACE: Color32 = Color32::from_rgb(30, 41, 59);
const ROBOT_EYE: Color32 = Color32::from_rgb(125, 211, 252);
const GHOST_BODY: Color32 = Color32::from_rgb(241, 245, 249);
const INK: Color32 = Color32::from_rgb(15, 23, 42);
const ZZZ_RGB: [u8; 3] = [147, 197, 253];

/// Point in unit space: (0,0) = centre, ±1 = half side `r`.
fn at(c: Pos2, r: f32, x: f32, y: f32) -> Pos2 {
    c + vec2(x, y) * r
}

fn rect_at(c: Pos2, r: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(at(c, r, x0, y0), at(c, r, x1, y1))
}

fn stroke_w(r: f32) -> f32 {
    (0.07 * r).max(1.0)
}

/// Three stroked "z" glyphs rising and fading in a loop, at unit point (x, y).
fn zzz(p: &Painter, c: Pos2, r: f32, x: f32, y: f32, t: f64) {
    for i in 0..3 {
        let ph = ((t * 0.4 + i as f64 / 3.0).fract()) as f32;
        let alpha = ((1.0 - ph) * 255.0) as u8;
        let col = Color32::from_rgba_unmultiplied(ZZZ_RGB[0], ZZZ_RGB[1], ZZZ_RGB[2], alpha);
        let fi = i as f32;
        let h = ((0.14 + 0.06 * fi) * r).max(2.0);
        let q = at(c, r, x + 0.15 * fi + 0.1 * ph, y - 0.3 * fi - 0.3 * ph);
        let pts = vec![q + vec2(-h, -h), q + vec2(h, -h), q + vec2(-h, h), q + vec2(h, h)];
        p.add(Shape::line(pts, Stroke::new(stroke_w(r), col)));
    }
}

/// Robot mascot: boxy head with antenna, dark face, arms, legs.
pub fn draw_robot(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    if !(s.is_finite() && s > 0.0) {
        return;
    }
    let r = s * 0.5;
    let bob = match pose {
        Pose::Idle | Pose::Error => 0.03 * (t * 1.6).sin() as f32,
        Pose::Working => 0.02 * (t * 9.0).sin() as f32,
        Pose::Sleeping => 0.08,
        Pose::Done => -0.06 * (t * 4.0).sin().abs() as f32,
    };
    let c = at(c, r, 0.0, bob);
    let (lift_l, lift_r) = match pose {
        Pose::Working => {
            let w = (t * 6.0).sin() as f32;
            (0.15 * w.max(0.0), 0.15 * (-w).max(0.0))
        }
        Pose::Done => (0.45, 0.45),
        _ => (0.0, 0.0),
    };
    let sw = stroke_w(r);

    // legs, body, arms
    p.rect_filled(rect_at(c, r, -0.32, 0.66, -0.08, 0.92), 0.05 * r, ROBOT_DARK);
    p.rect_filled(rect_at(c, r, 0.08, 0.66, 0.32, 0.92), 0.05 * r, ROBOT_DARK);
    p.rect_filled(rect_at(c, r, -0.45, 0.16, 0.45, 0.72), 0.1 * r, ROBOT_BODY);
    p.rect_filled(rect_at(c, r, -0.72, 0.22 - lift_l, -0.5, 0.62 - lift_l), 0.08 * r, ROBOT_DARK);
    p.rect_filled(rect_at(c, r, 0.5, 0.22 - lift_r, 0.72, 0.62 - lift_r), 0.08 * r, ROBOT_DARK);

    // head + antenna
    p.rect_filled(rect_at(c, r, -0.6, -0.5, 0.6, 0.12), 0.14 * r, ROBOT_BODY);
    p.line_segment([at(c, r, 0.0, -0.5), at(c, r, 0.0, -0.72)], Stroke::new(sw, ROBOT_DARK));
    p.circle_filled(at(c, r, 0.0, -0.8), 0.1 * r, ROBOT_EYE);

    // face + eyes
    p.rect_filled(rect_at(c, r, -0.42, -0.36, 0.42, 0.0), 0.08 * r, ROBOT_FACE);
    let look = if pose == Pose::Working { 0.05 * (t * 3.0).sin() as f32 } else { 0.0 };
    for sx in [-1.0f32, 1.0] {
        let ex = sx * 0.2 + look;
        match pose {
            Pose::Sleeping => {
                p.line_segment([at(c, r, ex - 0.12, -0.2), at(c, r, ex + 0.12, -0.2)], Stroke::new(sw, ROBOT_EYE));
            }
            Pose::Done => {
                p.line_segment([at(c, r, ex - 0.11, -0.1), at(c, r, ex, -0.24)], Stroke::new(sw, ROBOT_EYE));
                p.line_segment([at(c, r, ex, -0.24), at(c, r, ex + 0.11, -0.1)], Stroke::new(sw, ROBOT_EYE));
            }
            _ => {
                let blink = pose == Pose::Idle && (t % 4.0) > 3.85;
                let h = if blink { 0.02 } else { 0.14 };
                p.rect_filled(rect_at(c, r, ex - 0.08, -0.2 - h, ex + 0.08, -0.2 + h), 0.02 * r, ROBOT_EYE);
            }
        }
    }

    // mouth
    if pose == Pose::Done {
        p.line_segment([at(c, r, -0.2, -0.1), at(c, r, 0.0, -0.04)], Stroke::new(sw, ROBOT_EYE));
        p.line_segment([at(c, r, 0.0, -0.04), at(c, r, 0.2, -0.1)], Stroke::new(sw, ROBOT_EYE));
    } else {
        p.line_segment([at(c, r, -0.14, -0.08), at(c, r, 0.14, -0.08)], Stroke::new(sw, ROBOT_EYE));
    }

    if pose == Pose::Sleeping {
        zzz(p, c, r, 0.55, -0.55, t);
    }
}

/// Ghost mascot: round dome, wavy hem, dark eyes, faint ground shadow.
pub fn draw_ghost(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    if !(s.is_finite() && s > 0.0) {
        return;
    }
    let r = s * 0.5;
    // ground shadow stays put while the ghost floats
    p.add(Shape::ellipse_filled(
        at(c, r, 0.0, 0.92),
        vec2(0.5 * r, 0.08 * r),
        Color32::from_rgba_unmultiplied(0, 0, 0, 60),
    ));
    let bob = match pose {
        Pose::Idle | Pose::Error => 0.04 * (t * 1.6).sin() as f32,
        Pose::Working => 0.0,
        Pose::Sleeping => 0.1,
        Pose::Done => -0.08 * (t * 4.0).sin().abs() as f32,
    };
    let wob = if pose == Pose::Working { 0.06 * (t * 5.0).sin() as f32 } else { 0.0 };
    let c = at(c, r, wob, bob);
    let hem = |i: usize| -> f32 {
        if pose == Pose::Sleeping {
            0.0
        } else {
            0.06 * ((t * 4.0) + i as f64 * 1.5).sin() as f32
        }
    };

    // body: dome + rectangle + three hem bumps
    p.circle_filled(at(c, r, 0.0, -0.15), 0.6 * r, GHOST_BODY);
    p.rect_filled(rect_at(c, r, -0.6, -0.15, 0.6, 0.5), 0.0, GHOST_BODY);
    for (i, x) in [-0.4f32, 0.0, 0.4].into_iter().enumerate() {
        p.circle_filled(at(c, r, x, 0.5 + hem(i)), 0.2 * r, GHOST_BODY);
    }

    // eyes
    let look = if pose == Pose::Working { 0.04 * (t * 3.0).sin() as f32 } else { 0.0 };
    let sw = stroke_w(r);
    for sx in [-1.0f32, 1.0] {
        let ex = sx * 0.22 + look;
        match pose {
            Pose::Sleeping => {
                p.line_segment([at(c, r, ex - 0.1, -0.1), at(c, r, ex + 0.1, -0.1)], Stroke::new(sw, INK));
            }
            Pose::Done => {
                p.line_segment([at(c, r, ex - 0.1, -0.02), at(c, r, ex, -0.16)], Stroke::new(sw, INK));
                p.line_segment([at(c, r, ex, -0.16), at(c, r, ex + 0.1, -0.02)], Stroke::new(sw, INK));
            }
            _ => {
                let blink = pose == Pose::Idle && (t % 4.0) > 3.85;
                let ry = if blink { 0.02 } else { 0.13 };
                p.add(Shape::ellipse_filled(at(c, r, ex, -0.1), vec2(0.1 * r, ry * r), INK));
            }
        }
    }

    // mouth
    match pose {
        Pose::Sleeping => {
            p.add(Shape::ellipse_filled(at(c, r, 0.0, 0.2), vec2(0.05 * r, 0.05 * r), INK));
        }
        Pose::Done => {
            p.line_segment([at(c, r, -0.22, 0.12), at(c, r, 0.0, 0.3)], Stroke::new(sw, INK));
            p.line_segment([at(c, r, 0.0, 0.3), at(c, r, 0.22, 0.12)], Stroke::new(sw, INK));
        }
        Pose::Working => {
            p.line_segment([at(c, r, -0.1, 0.22), at(c, r, 0.1, 0.22)], Stroke::new(sw, INK));
        }
        Pose::Idle | Pose::Error => {
            p.add(Shape::ellipse_filled(at(c, r, 0.0, 0.22), vec2(0.09 * r, 0.12 * r), INK));
        }
    }

    if pose == Pose::Sleeping {
        zzz(p, c, r, 0.6, -0.6, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, LayerId, RawInput, Shape as S};
    use std::collections::HashSet;

    const SIZES: [f32; 3] = [24.0, 40.0, 56.0];
    const POSES: [Pose; 4] = [Pose::Idle, Pose::Working, Pose::Sleeping, Pose::Done];
    const TIMES: [f64; 3] = [0.0, 1.3, 2.7];
    const CENTRE: Pos2 = Pos2::new(100.0, 100.0);

    /// Run `f` on a headless egui context; return every shape painted on the background layer.
    fn capture(f: impl Fn(&Painter)) -> Vec<S> {
        let ctx = Context::default();
        let mut out = Vec::new();
        let _ = ctx.run(RawInput::default(), |ctx| {
            let layer = LayerId::background();
            f(&ctx.layer_painter(layer));
            ctx.graphics(|g| {
                if let Some(list) = g.get(layer) {
                    out = list.all_entries().map(|cs| cs.shape.clone()).collect();
                }
            });
        });
        out
    }

    fn fills(shapes: &[S]) -> Vec<Color32> {
        fn walk(s: &S, out: &mut Vec<Color32>) {
            match s {
                S::Vec(v) => v.iter().for_each(|x| walk(x, out)),
                S::Circle(c) => out.push(c.fill),
                S::Ellipse(e) => out.push(e.fill),
                S::Rect(r) => out.push(r.fill),
                _ => {}
            }
        }
        let mut out = Vec::new();
        shapes.iter().for_each(|s| walk(s, &mut out));
        out
    }

    fn paths(shapes: &[S]) -> usize {
        shapes.iter().filter(|s| matches!(s, S::Path(_))).count()
    }

    fn within(shapes: &[S], c: Pos2, s: f32) -> bool {
        shapes.iter().all(|sh| {
            let b = sh.visual_bounding_rect();
            b.min.x.is_finite()
                && b.min.y.is_finite()
                && b.max.x.is_finite()
                && b.max.y.is_finite()
                && b.min.x >= c.x - s
                && b.min.y >= c.y - s
                && b.max.x <= c.x + s
                && b.max.y <= c.y + s
        })
    }

    #[test]
    fn robot_paints_every_pose_and_time_at_every_size() {
        for &s in &SIZES {
            for pose in POSES {
                for &t in &TIMES {
                    let shapes = capture(|p| draw_robot(p, CENTRE, s, pose, t));
                    assert!(!shapes.is_empty(), "robot empty s={s} {pose:?}");
                    assert!(fills(&shapes).contains(&ROBOT_BODY), "robot body missing s={s} {pose:?}");
                    assert!(within(&shapes, CENTRE, s), "robot out of bounds s={s} {pose:?} t={t}");
                }
            }
        }
    }

    #[test]
    fn ghost_paints_every_pose_and_time_at_every_size() {
        for &s in &SIZES {
            for pose in POSES {
                for &t in &TIMES {
                    let shapes = capture(|p| draw_ghost(p, CENTRE, s, pose, t));
                    assert!(!shapes.is_empty(), "ghost empty s={s} {pose:?}");
                    assert!(fills(&shapes).contains(&GHOST_BODY), "ghost body missing s={s} {pose:?}");
                    assert!(within(&shapes, CENTRE, s), "ghost out of bounds s={s} {pose:?} t={t}");
                }
            }
        }
    }

    #[test]
    fn robot_and_ghost_are_distinct_at_every_size() {
        for &s in &SIZES {
            let r = fills(&capture(|p| draw_robot(p, CENTRE, s, Pose::Idle, 0.0)));
            let g = fills(&capture(|p| draw_ghost(p, CENTRE, s, Pose::Idle, 0.0)));
            assert!(!r.contains(&GHOST_BODY), "robot uses ghost colour s={s}");
            assert!(!g.contains(&ROBOT_BODY), "ghost uses robot colour s={s}");
        }
    }

    #[test]
    fn sleeping_shows_three_zzz_only_when_sleeping() {
        for &s in &SIZES {
            for &t in &TIMES {
                let rs = capture(|p| draw_robot(p, CENTRE, s, Pose::Sleeping, t));
                let gs = capture(|p| draw_ghost(p, CENTRE, s, Pose::Sleeping, t));
                assert_eq!(paths(&rs), 3, "robot zzz s={s} t={t}");
                assert_eq!(paths(&gs), 3, "ghost zzz s={s} t={t}");
            }
            for pose in [Pose::Idle, Pose::Working, Pose::Done] {
                assert_eq!(paths(&capture(|p| draw_robot(p, CENTRE, s, pose, 0.5))), 0);
                assert_eq!(paths(&capture(|p| draw_ghost(p, CENTRE, s, pose, 0.5))), 0);
            }
        }
    }

    #[test]
    fn four_poses_give_four_geometries_and_animate_with_t() {
        for &s in &SIZES {
            let mut robot = HashSet::new();
            let mut ghost = HashSet::new();
            for pose in POSES {
                robot.insert(format!("{:?}", capture(|p| draw_robot(p, CENTRE, s, pose, 0.5))));
                ghost.insert(format!("{:?}", capture(|p| draw_ghost(p, CENTRE, s, pose, 0.5))));
            }
            assert_eq!(robot.len(), 4, "robot poses not distinct s={s}");
            assert_eq!(ghost.len(), 4, "ghost poses not distinct s={s}");
            let a = format!("{:?}", capture(|p| draw_robot(p, CENTRE, s, Pose::Working, 0.0)));
            let b = format!("{:?}", capture(|p| draw_robot(p, CENTRE, s, Pose::Working, 0.1)));
            assert_ne!(a, b, "robot not animated s={s}");
            let a = format!("{:?}", capture(|p| draw_ghost(p, CENTRE, s, Pose::Working, 0.0)));
            let b = format!("{:?}", capture(|p| draw_ghost(p, CENTRE, s, Pose::Working, 0.1)));
            assert_ne!(a, b, "ghost not animated s={s}");
        }
    }

    #[test]
    fn degenerate_size_paints_nothing() {
        assert!(capture(|p| draw_robot(p, CENTRE, 0.0, Pose::Idle, 0.0)).is_empty());
        assert!(capture(|p| draw_ghost(p, CENTRE, f32::NAN, Pose::Idle, 0.0)).is_empty());
    }
}
