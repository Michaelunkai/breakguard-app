//! slice creat_frog_penguin (owned by one team agent)
//! API: pub fn draw_frog(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! API: pub fn draw_penguin(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! Poses: Idle (breathes, blinks), Working (bobs, eyes look down-left), Sleeping (closed eyes, zzz), Done (^ eyes, hop / raised flippers).
//! Painter primitives only. `s` = square side in px, `t` = seconds.
#![allow(dead_code)]
use crate::ui::creatures::Pose;
use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Shape, Stroke, Vec2};

const INK: Color32 = Color32::from_rgb(15, 23, 42);
const FROG: Color32 = Color32::from_rgb(74, 190, 100);
const FROG_PALE: Color32 = Color32::from_rgb(190, 240, 170);
const PENGUIN: Color32 = Color32::from_rgb(30, 41, 59);
const PENGUIN_BELLY: Color32 = Color32::from_rgb(248, 250, 252);
const BEAK: Color32 = Color32::from_rgb(251, 146, 60);

/// Pose-dependent whole-body offset and scale. Returns (centre, scale).
fn motion(c: Pos2, s: f32, pose: Pose, t: f64) -> (Pos2, f32) {
    match pose {
        Pose::Idle | Pose::Error => (c, 1.0 + 0.03 * (t * 1.6).sin() as f32),
        Pose::Working => (c + vec2(0.0, ((t * 9.0).sin() as f32) * s * 0.025), 1.0),
        Pose::Sleeping => (c + vec2(0.0, s * 0.05), 1.0 + 0.015 * (t * 1.0).sin() as f32),
        Pose::Done => (c - vec2(0.0, (t * 4.0).sin().abs() as f32 * s * 0.06), 1.0),
    }
}

/// One eye at `e` with white radius `rad`. `look` shifts the pupil (in units of rad).
fn eye(p: &Painter, e: Pos2, rad: f32, pose: Pose, t: f64, look: Vec2, white: Color32) {
    let w = (rad * 0.35).max(1.0);
    let line = Stroke::new(w, INK);
    match pose {
        Pose::Sleeping => {
            p.line_segment([e - vec2(rad, 0.0), e + vec2(rad, 0.0)], line);
        }
        Pose::Done => {
            p.line_segment([e + vec2(-rad, 0.4 * rad), e + vec2(0.0, -0.5 * rad)], line);
            p.line_segment([e + vec2(0.0, -0.5 * rad), e + vec2(rad, 0.4 * rad)], line);
        }
        _ => {
            if pose == Pose::Idle && (t % 4.0) > 3.85 {
                p.line_segment([e - vec2(rad, 0.0), e + vec2(rad, 0.0)], line);
            } else {
                p.circle_filled(e, rad, white);
                p.circle_filled(e + look * rad * 0.45, rad * 0.5, INK);
            }
        }
    }
}

/// Idle pupils wander slightly; Working pupils look down-left; Done/Sleeping have none.
fn look_for(pose: Pose, t: f64) -> Vec2 {
    match pose {
        Pose::Idle | Pose::Error => vec2((t * 0.7).sin() as f32 * 0.3, 0.0),
        Pose::Working => vec2(-0.6, 0.6),
        _ => Vec2::ZERO,
    }
}

fn zzz(p: &Painter, c: Pos2, s: f32, t: f64) {
    let f = FontId::proportional((s * 0.28).max(9.0));
    let k = (t * 0.8) as f32 % 1.0;
    p.text(
        pos2(c.x + s * 0.45, c.y - s * (0.35 + 0.2 * k)),
        Align2::CENTER_CENTER,
        "z",
        f.clone(),
        Color32::from_white_alpha((255.0 * (1.0 - k)) as u8),
    );
    p.text(pos2(c.x + s * 0.62, c.y - s * 0.55), Align2::CENTER_CENTER, "Z", f, Color32::from_white_alpha(200));
}

/// Frog: wide green body, big pop eyes on top of the head, pale belly, wide mouth.
pub fn draw_frog(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    let (c, k) = motion(c, s, pose, t);
    let r = s * 0.5 * k;
    let at = |x: f32, y: f32| c + vec2(x, y) * r;
    let lw = (0.07 * r).max(1.0);

    // hind legs
    p.add(Shape::ellipse_filled(at(-0.8, 0.75), vec2(0.28, 0.16) * r, FROG));
    p.add(Shape::ellipse_filled(at(0.8, 0.75), vec2(0.28, 0.16) * r, FROG));
    // body
    p.add(Shape::ellipse_filled(at(0.0, 0.25), vec2(0.95, 0.65) * r, FROG));
    // pop eyes
    p.circle_filled(at(-0.45, -0.5), 0.4 * r, FROG);
    p.circle_filled(at(0.45, -0.5), 0.4 * r, FROG);
    // belly
    p.add(Shape::ellipse_filled(at(0.0, 0.55), vec2(0.5, 0.22) * r, FROG_PALE));
    // eyes
    let white = Color32::from_rgb(248, 250, 252);
    let look = look_for(pose, t);
    eye(p, at(-0.45, -0.5), 0.26 * r, pose, t, look, white);
    eye(p, at(0.45, -0.5), 0.26 * r, pose, t, look, white);
    // mouth
    let mouth = match pose {
        Pose::Done => [at(-0.6, 0.0), at(0.0, 0.4), at(0.6, 0.0)],
        _ => [at(-0.5, 0.05), at(0.0, 0.2), at(0.5, 0.05)],
    };
    p.line_segment([mouth[0], mouth[1]], Stroke::new(lw, INK));
    p.line_segment([mouth[1], mouth[2]], Stroke::new(lw, INK));

    if pose == Pose::Sleeping {
        zzz(p, c, s, t);
    }
}

/// Penguin: tall dark body, white belly, orange beak and feet, flippers at the sides.
pub fn draw_penguin(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    let (c, k) = motion(c, s, pose, t);
    let r = s * 0.5 * k;
    let at = |x: f32, y: f32| c + vec2(x, y) * r;

    // feet
    p.add(Shape::ellipse_filled(at(-0.3, 0.93), vec2(0.22, 0.1) * r, BEAK));
    p.add(Shape::ellipse_filled(at(0.3, 0.93), vec2(0.22, 0.1) * r, BEAK));
    // body
    p.add(Shape::ellipse_filled(at(0.0, 0.05), vec2(0.72, 0.9) * r, PENGUIN));
    // belly
    p.add(Shape::ellipse_filled(at(0.0, 0.2), vec2(0.45, 0.66) * r, PENGUIN_BELLY));
    // flippers
    let flip = |side: f32, lift: f32| -> Vec<Pos2> {
        vec![at(side * 0.7, -0.2), at(side * (0.98 + lift), 0.3 - lift), at(side * 0.62, 0.35)]
    };
    let (l, rt) = match pose {
        Pose::Done => (flip(-1.0, 0.35), flip(1.0, 0.35)),
        Pose::Working => {
            let tap = ((t * 8.0).sin() as f32) * 0.15;
            (flip(-1.0, 0.0), flip(1.0, tap))
        }
        _ => (flip(-1.0, 0.0), flip(1.0, 0.0)),
    };
    p.add(Shape::convex_polygon(l, PENGUIN, Stroke::NONE));
    p.add(Shape::convex_polygon(rt, PENGUIN, Stroke::NONE));
    // eyes
    let look = look_for(pose, t);
    eye(p, at(-0.25, -0.5), 0.16 * r, pose, t, look, PENGUIN_BELLY);
    eye(p, at(0.25, -0.5), 0.16 * r, pose, t, look, PENGUIN_BELLY);
    // beak
    p.add(Shape::convex_polygon(vec![at(-0.14, -0.22), at(0.14, -0.22), at(0.0, 0.02)], BEAK, Stroke::NONE));

    if pose == Pose::Sleeping {
        zzz(p, c, s, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, LayerId, RawInput, Rect};

    const SIZES: [f32; 3] = [24.0, 40.0, 56.0];
    const POSES: [Pose; 4] = [Pose::Idle, Pose::Working, Pose::Sleeping, Pose::Done];
    const TIMES: [f64; 3] = [0.0, 1.3, 3.9];

    /// Run `draw` on a headless egui context and return the emitted shapes.
    fn render(draw: impl Fn(&Painter)) -> Vec<Shape> {
        let ctx = Context::default();
        let mut shapes = Vec::new();
        let out = ctx.run(RawInput::default(), |ctx| {
            let p = ctx.layer_painter(LayerId::background());
            draw(&p);
        });
        for cs in out.shapes {
            shapes.push(cs.shape);
        }
        shapes
    }

    fn bounds(shapes: &[Shape]) -> Rect {
        shapes.iter().fold(Rect::NOTHING, |acc, s| acc.union(s.visual_bounding_rect()))
    }

    #[test]
    fn frog_draws_every_pose_size_and_time_within_box() {
        let c = pos2(100.0, 100.0);
        for &s in &SIZES {
            for &pose in &POSES {
                for &t in &TIMES {
                    let shapes = render(|p| draw_frog(p, c, s, pose, t));
                    assert!(shapes.len() >= 8, "frog s={s} {pose:?} t={t}: {} shapes", shapes.len());
                    if pose != Pose::Sleeping {
                        let b = bounds(&shapes);
                        assert!(b.width() <= s * 1.4 && b.height() <= s * 1.4, "frog overflows box s={s} {pose:?}: {b:?}");
                        assert!(b.center().distance(c) < s * 0.3, "frog off centre s={s} {pose:?}: {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn penguin_draws_every_pose_size_and_time_within_box() {
        let c = pos2(100.0, 100.0);
        for &s in &SIZES {
            for &pose in &POSES {
                for &t in &TIMES {
                    let shapes = render(|p| draw_penguin(p, c, s, pose, t));
                    assert!(shapes.len() >= 8, "penguin s={s} {pose:?} t={t}: {} shapes", shapes.len());
                    if pose != Pose::Sleeping {
                        let b = bounds(&shapes);
                        assert!(b.width() <= s * 1.4 && b.height() <= s * 1.4, "penguin overflows box s={s} {pose:?}: {b:?}");
                        assert!(b.center().distance(c) < s * 0.3, "penguin off centre s={s} {pose:?}: {b:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn frog_and_penguin_are_distinct_at_every_size() {
        let c = pos2(50.0, 50.0);
        for &s in &SIZES {
            for &pose in &POSES {
                let f = format!("{:?}", render(|p| draw_frog(p, c, s, pose, 0.5)));
                let g = format!("{:?}", render(|p| draw_penguin(p, c, s, pose, 0.5)));
                assert_ne!(f, g, "frog == penguin s={s} {pose:?}");
            }
        }
    }

    #[test]
    fn sleeping_adds_zzz_text() {
        let c = pos2(50.0, 50.0);
        for &s in &SIZES {
            let fz = render(|p| draw_frog(p, c, s, Pose::Sleeping, 0.0));
            let pz = render(|p| draw_penguin(p, c, s, Pose::Sleeping, 0.0));
            assert!(fz.iter().any(|x| matches!(x, Shape::Text(_))), "frog no zzz s={s}");
            assert!(pz.iter().any(|x| matches!(x, Shape::Text(_))), "penguin no zzz s={s}");
        }
    }

    #[test]
    fn animation_changes_output_over_time() {
        let c = pos2(50.0, 50.0);
        for &s in &SIZES {
            let a = format!("{:?}", render(|p| draw_frog(p, c, s, Pose::Done, 0.0)));
            let b = format!("{:?}", render(|p| draw_frog(p, c, s, Pose::Done, 0.5)));
            assert_ne!(a, b, "frog Done static s={s}");
        }
    }
}
