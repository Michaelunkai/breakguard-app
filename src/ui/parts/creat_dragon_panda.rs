//! slice creat_dragon_panda (owned by one team agent)
//! API: pub fn draw_dragon(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! API: pub fn draw_panda(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! Painter primitives only (no assets). `c` = centre, `s` = square side in px, `t` = seconds.
#![allow(dead_code)]
use crate::ui::creatures::Pose;
use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Shape, Stroke};

const INK: Color32 = Color32::from_rgb(15, 23, 42);
/// Light grey rim so black panda parts stay visible on the dark theme.
const RIM: Color32 = Color32::from_rgb(148, 163, 184);
const WHITE: Color32 = Color32::from_rgb(248, 250, 252);

/// Pose-dependent centre and radius (same motion language as creatures.rs).
fn placed(c: Pos2, s: f32, pose: Pose, t: f64) -> (Pos2, f32) {
    let mut c = c;
    let mut r = s * 0.5;
    match pose {
        Pose::Idle | Pose::Error => r *= 1.0 + 0.03 * (t * 1.6).sin() as f32,
        Pose::Working => c.y += ((t * 9.0).sin() as f32) * s * 0.025,
        Pose::Sleeping => c.y += s * 0.05,
        Pose::Done => c.y -= ((t * 4.0).sin().abs() as f32) * s * 0.03,
    }
    (c, r)
}

fn poly(p: &Painter, c: Pos2, r: f32, pts: &[(f32, f32)], fill: Color32) {
    p.add(Shape::convex_polygon(pts.iter().map(|(x, y)| c + vec2(*x, *y) * r).collect(), fill, Stroke::NONE));
}

fn dot(p: &Painter, c: Pos2, r: f32, x: f32, y: f32, rr: f32, fill: Color32) {
    p.circle_filled(c + vec2(x, y) * r, rr * r, fill);
}

fn ellipse(p: &Painter, c: Pos2, r: f32, x: f32, y: f32, rx: f32, ry: f32, fill: Color32) {
    p.add(Shape::ellipse_filled(c + vec2(x, y) * r, vec2(rx, ry) * r, fill));
}

/// Oval rotated by `angle` radians around its centre (polygon, so it can tilt).
fn tilted_oval(p: &Painter, c: Pos2, r: f32, x: f32, y: f32, rx: f32, ry: f32, angle: f32, fill: Color32) {
    let (sin, cos) = angle.sin_cos();
    let pts: Vec<(f32, f32)> = (0..24)
        .map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let (ex, ey) = (a.cos() * rx, a.sin() * ry);
            (x + ex * cos - ey * sin, y + ex * sin + ey * cos)
        })
        .collect();
    poly(p, c, r, &pts, fill);
}

fn stroke_w(r: f32) -> f32 {
    (0.07 * r).max(1.0)
}

/// Two eyes at (±dx, y). Sleeping = closed lines, Done = happy ^ arcs, Idle blinks, Working looks around.
fn eyes(p: &Painter, c: Pos2, r: f32, y: f32, dx: f32, size: f32, pose: Pose, t: f64) {
    for sgn in [-1.0f32, 1.0] {
        let e = c + vec2(sgn * dx, y) * r;
        match pose {
            Pose::Sleeping => {
                p.line_segment([e - vec2(size * r, 0.0), e + vec2(size * r, 0.0)], Stroke::new(stroke_w(r), INK));
            }
            Pose::Done => {
                let w = size * r;
                p.line_segment([e + vec2(-w, 0.3 * w), e + vec2(0.0, -0.5 * w)], Stroke::new(stroke_w(r), INK));
                p.line_segment([e + vec2(0.0, -0.5 * w), e + vec2(w, 0.3 * w)], Stroke::new(stroke_w(r), INK));
            }
            _ => {
                let blink = pose == Pose::Idle && (t % 4.0) > 3.85;
                let h = if blink { 0.15 } else { 1.0 };
                p.add(Shape::ellipse_filled(e, vec2(size, size * h) * r, WHITE));
                if !blink {
                    let look = if pose == Pose::Working { ((t * 3.0).sin() as f32) * 0.3 } else { 0.0 };
                    p.circle_filled(e + vec2(look * size * r, 0.0), size * 0.55 * r, INK);
                }
            }
        }
    }
}

/// Drifting "z" / "Z" next to the head while sleeping.
fn zzz(p: &Painter, c: Pos2, s: f32, t: f64) {
    let f = FontId::proportional((s * 0.28).max(9.0));
    let k = ((t * 0.8) as f32).rem_euclid(1.0);
    let a = (255.0 * (1.0 - k)) as u8;
    p.text(pos2(c.x + s * 0.45, c.y - s * (0.35 + 0.2 * k)), Align2::CENTER_CENTER, "z", f.clone(), Color32::from_white_alpha(a));
    p.text(pos2(c.x + s * 0.62, c.y - s * 0.55), Align2::CENTER_CENTER, "Z", f, Color32::from_white_alpha(200));
}

/// Green dragon: horns, bat wings (flap while Working, raised when Done), tail, snout.
pub fn draw_dragon(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    let (c, r) = placed(c, s, pose, t);
    let green = Color32::from_rgb(22, 163, 74);
    let dark = Color32::from_rgb(21, 128, 61);
    let belly = Color32::from_rgb(187, 247, 208);
    let horn = Color32::from_rgb(251, 191, 36);
    let wing = Color32::from_rgb(124, 58, 237);

    // Tail: polyline with a spike tip, drawn first so body covers its root.
    p.line(
        vec![c + vec2(0.2, 0.6) * r, c + vec2(0.6, 0.85) * r, c + vec2(0.9, 0.7) * r, c + vec2(1.0, 0.4) * r],
        Stroke::new(0.14 * r, dark),
    );
    poly(p, c, r, &[(0.95, 0.5), (1.12, 0.35), (1.0, 0.7)], dark);

    // Wings: flap angle while Working, fully raised when Done.
    let lift = match pose {
        Pose::Working => ((t * 6.0).sin() as f32) * 0.25,
        Pose::Done => 0.35,
        _ => 0.0,
    };
    for sgn in [-1.0f32, 1.0] {
        let pts = [(-0.4, -0.15), (-1.0, -0.8 - lift), (-0.95, 0.05), (-0.5, 0.3)];
        let mapped: Vec<(f32, f32)> = pts.iter().map(|(x, y)| (x * sgn, *y)).collect();
        poly(p, c, r, &mapped, wing);
    }

    // Feet, body, belly.
    dot(p, c, r, -0.3, 0.88, 0.14, dark);
    dot(p, c, r, 0.3, 0.88, 0.14, dark);
    ellipse(p, c, r, 0.0, 0.2, 0.62, 0.7, green);
    ellipse(p, c, r, 0.0, 0.3, 0.36, 0.45, belly);

    // Head, snout, nostrils.
    dot(p, c, r, 0.0, -0.5, 0.5, green);
    ellipse(p, c, r, 0.2, -0.3, 0.3, 0.2, green);
    dot(p, c, r, 0.3, -0.36, 0.03, INK);
    dot(p, c, r, 0.14, -0.36, 0.03, INK);

    // Horns.
    for sgn in [-1.0f32, 1.0] {
        poly(p, c, r, &[(0.3 * sgn, -0.8), (0.45 * sgn, -1.1), (0.1 * sgn, -0.9)], horn);
    }

    eyes(p, c, r, -0.55, 0.22, 0.13, pose, t);
    if pose == Pose::Sleeping {
        zzz(p, c, s, t);
    }
}

/// Black-and-white panda: round ears, eye patches, muzzle, arms raised when Done.
pub fn draw_panda(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    let (c, r) = placed(c, s, pose, t);
    let ink = INK;

    // Body and arms (arms lift while Done, swing slightly while Working).
    ellipse(p, c, r, 0.0, 0.75, 0.5, 0.45, WHITE);
    let arm_y = match pose {
        Pose::Done => -0.35,
        Pose::Working => 0.6 + ((t * 9.0).sin() as f32) * 0.05,
        _ => 0.6,
    };
    ellipse(p, c, r, -0.55, arm_y, 0.24, 0.34, RIM);
    ellipse(p, c, r, 0.55, arm_y, 0.24, 0.34, RIM);
    dot(p, c, r, -0.25, 1.1, 0.2, RIM);
    dot(p, c, r, 0.25, 1.1, 0.2, RIM);
    ellipse(p, c, r, -0.55, arm_y, 0.2, 0.3, ink);
    ellipse(p, c, r, 0.55, arm_y, 0.2, 0.3, ink);
    dot(p, c, r, -0.25, 1.1, 0.16, ink);
    dot(p, c, r, 0.25, 1.1, 0.16, ink);

    // Ears, head, eye patches.
    dot(p, c, r, -0.62, -0.62, 0.35, RIM);
    dot(p, c, r, 0.62, -0.62, 0.35, RIM);
    dot(p, c, r, -0.62, -0.62, 0.3, ink);
    dot(p, c, r, 0.62, -0.62, 0.3, ink);
    dot(p, c, r, 0.0, -0.1, 0.85, WHITE);
    tilted_oval(p, c, r, -0.38, -0.12, 0.22, 0.3, -0.35, ink);
    tilted_oval(p, c, r, 0.38, -0.12, 0.22, 0.3, 0.35, ink);
    eyes(p, c, r, -0.12, 0.38, 0.1, pose, t);

    // Nose only (no mouth lines: they read as a skull's cross mark at small sizes).
    ellipse(p, c, r, 0.0, 0.2, 0.12, 0.09, ink);

    if pose == Pose::Sleeping {
        zzz(p, c, s, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, LayerId, RawInput};

    const POSES: [Pose; 4] = [Pose::Idle, Pose::Working, Pose::Sleeping, Pose::Done];
    const SIZES: [f32; 3] = [24.0, 40.0, 56.0];

    /// Runs `f` for every pose and size on a headless egui context; returns total shape count.
    fn run_all(f: fn(&Painter, Pos2, f32, Pose, f64)) -> usize {
        let ctx = Context::default();
        let mut total = 0;
        for pose in POSES {
            for s in SIZES {
                for t in [0.0f64, 1.3, -3.7, 1.0e6] {
                    let out = ctx.run(RawInput::default(), |ctx| {
                        let p = ctx.layer_painter(LayerId::background());
                        f(&p, pos2(100.0, 100.0), s, pose, t);
                    });
                    assert!(!out.shapes.is_empty(), "no shapes for pose={pose:?} s={s} t={t}");
                    total += out.shapes.len();
                }
            }
        }
        total
    }

    #[test]
    fn draw_dragon_runs_all_poses_and_sizes() {
        assert!(run_all(draw_dragon) > 0);
    }

    #[test]
    fn draw_panda_runs_all_poses_and_sizes() {
        assert!(run_all(draw_panda) > 0);
    }

    #[test]
    fn dragon_and_panda_produce_different_shape_counts() {
        // Distinct silhouettes: a cheap guard against the two drawers collapsing into one.
        let ctx = Context::default();
        let count = |f: fn(&Painter, Pos2, f32, Pose, f64)| {
            let out = ctx.run(RawInput::default(), |ctx| {
                let p = ctx.layer_painter(LayerId::background());
                f(&p, pos2(0.0, 0.0), 40.0, Pose::Idle, 0.0);
            });
            out.shapes.len()
        };
        assert_ne!(count(draw_dragon), count(draw_panda));
    }
}
