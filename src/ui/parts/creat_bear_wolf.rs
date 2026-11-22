//! slice creat_bear_wolf (owned by one team agent)
//! API: pub fn draw_bear(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! API: pub fn draw_wolf(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64)
//! Painter primitives only. `c` is the centre, `s` the square side in px, `t` seconds.
//! Poses: Idle (bob + blink), Working (fast bob + eyes scan), Sleeping (closed eyes + zzz), Done (happy arcs + smile).
#![allow(dead_code)]
use crate::ui::creatures::Pose;
use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Shape, Stroke};

pub const BEAR_FUR: Color32 = Color32::from_rgb(139, 90, 43);
pub const BEAR_INNER: Color32 = Color32::from_rgb(101, 62, 28);
pub const BEAR_MUZZLE: Color32 = Color32::from_rgb(222, 184, 135);
pub const WOLF_FUR: Color32 = Color32::from_rgb(120, 130, 145);
pub const WOLF_LIGHT: Color32 = Color32::from_rgb(226, 232, 240);
pub const WOLF_EYE: Color32 = Color32::from_rgb(250, 204, 21);
const INK: Color32 = Color32::from_rgb(15, 23, 42);
const WHITE: Color32 = Color32::from_rgb(248, 250, 252);

/// Unit-space helper: (-1..1) across the square, bobbed by pose.
struct Frame {
    c: Pos2,
    r: f32,
    bob: f32,
}

impl Frame {
    fn new(c: Pos2, s: f32, pose: Pose, t: f64) -> Self {
        let bob = match pose {
            Pose::Idle | Pose::Error => (t * 1.6).sin() as f32 * 0.03,
            Pose::Working => (t * 9.0).sin() as f32 * 0.025,
            Pose::Sleeping => 0.05,
            Pose::Done => -((t * 4.0).sin().abs()) as f32 * 0.06,
        };
        Self { c, r: s * 0.5, bob }
    }
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.c + vec2(x, y + self.bob) * self.r
    }
    fn stroke(&self) -> Stroke {
        Stroke::new((0.07 * self.r).max(1.0), INK)
    }
    fn circle(&self, p: &Painter, x: f32, y: f32, rr: f32, fill: Color32) {
        p.circle_filled(self.pt(x, y), rr * self.r, fill);
    }
    fn ellipse(&self, p: &Painter, x: f32, y: f32, rx: f32, ry: f32, fill: Color32) {
        p.add(Shape::ellipse_filled(self.pt(x, y), vec2(rx, ry) * self.r, fill));
    }
    fn tri(&self, p: &Painter, pts: [(f32, f32); 3], fill: Color32) {
        let v = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        p.add(Shape::convex_polygon(v, fill, Stroke::NONE));
    }
}

fn line(p: &Painter, pts: Vec<Pos2>, stroke: Stroke) {
    p.add(Shape::line(pts, stroke));
}

/// Eyes at x = ±dx, height y, radius r (unit). `fill` is the eye white colour.
fn eyes(p: &Painter, f: &Frame, pose: Pose, t: f64, dx: f32, y: f32, r: f32, fill: Color32) {
    let stroke = f.stroke();
    for side in [-1.0f32, 1.0] {
        let x = side * dx;
        match pose {
            Pose::Sleeping => line(p, vec![f.pt(x - r, y), f.pt(x + r, y)], stroke),
            Pose::Done => line(
                p,
                vec![f.pt(x - r, y + r * 0.4), f.pt(x, y - r * 0.5), f.pt(x + r, y + r * 0.4)],
                stroke,
            ),
            _ => {
                let blink = pose == Pose::Idle && (t % 4.0) > 3.85;
                if blink {
                    line(p, vec![f.pt(x - r, y), f.pt(x + r, y)], stroke);
                    continue;
                }
                let look = if pose == Pose::Working { (t * 3.0).sin() as f32 * 0.4 * r } else { 0.0 };
                f.circle(p, x, y, r, fill);
                f.circle(p, x + look, y, r * 0.55, INK);
            }
        }
    }
}

fn smile(p: &Painter, f: &Frame) {
    line(p, vec![f.pt(-0.12, 0.22), f.pt(0.0, 0.3), f.pt(0.12, 0.22)], f.stroke());
}

/// Three rising z glyphs drawn in screen space (no bob), staggered and fading.
fn zzz(p: &Painter, f: &Frame, t: f64) {
    for i in 0..3 {
        let ph = ((t * 0.4 + i as f64 / 3.0).fract()) as f32;
        let k = 0.25 + 0.08 * i as f32;
        let ox = f.c.x + (0.4 + 0.2 * ph) * f.r;
        let oy = f.c.y + (-0.3 - 0.6 * ph) * f.r;
        let w = k * f.r;
        let alpha = ((1.0 - ph) * 255.0 * (ph * 4.0).min(1.0)) as u8;
        let col = Color32::from_rgba_unmultiplied(INK.r(), INK.g(), INK.b(), alpha);
        let stroke = Stroke::new((0.06 * f.r).max(1.0), col);
        line(
            p,
            vec![pos2(ox, oy), pos2(ox + w, oy), pos2(ox, oy + w), pos2(ox + w, oy + w)],
            stroke,
        );
    }
}

/// Bear: round ears, round head, tan muzzle, short snout.
pub fn draw_bear(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    let f = Frame::new(c, s, pose, t);
    f.circle(p, -0.42, -0.58, 0.22, BEAR_FUR);
    f.circle(p, 0.42, -0.58, 0.22, BEAR_FUR);
    f.circle(p, -0.42, -0.58, 0.11, BEAR_INNER);
    f.circle(p, 0.42, -0.58, 0.11, BEAR_INNER);
    f.ellipse(p, 0.0, 0.62, 0.5, 0.36, BEAR_FUR);
    f.circle(p, 0.0, -0.08, 0.56, BEAR_FUR);
    f.ellipse(p, 0.0, 0.14, 0.27, 0.2, BEAR_MUZZLE);
    f.ellipse(p, 0.0, 0.04, 0.09, 0.065, INK);
    eyes(p, &f, pose, t, 0.22, -0.18, 0.085, WHITE);
    if pose == Pose::Done {
        smile(p, &f);
    }
    if pose == Pose::Sleeping {
        zzz(p, &f, t);
    }
}

/// Wolf: pointed ears, grey head, long pointed snout, amber eyes, angled brows.
pub fn draw_wolf(p: &Painter, c: Pos2, s: f32, pose: Pose, t: f64) {
    let f = Frame::new(c, s, pose, t);
    f.tri(p, [(-0.52, -0.12), (-0.5, -0.88), (-0.08, -0.38)], WOLF_FUR);
    f.tri(p, [(0.52, -0.12), (0.5, -0.88), (0.08, -0.38)], WOLF_FUR);
    f.tri(p, [(-0.42, -0.2), (-0.42, -0.72), (-0.15, -0.36)], WOLF_LIGHT);
    f.tri(p, [(0.42, -0.2), (0.42, -0.72), (0.15, -0.36)], WOLF_LIGHT);
    f.ellipse(p, 0.0, 0.62, 0.44, 0.36, WOLF_FUR);
    f.ellipse(p, 0.0, 0.7, 0.22, 0.22, WOLF_LIGHT);
    f.ellipse(p, 0.0, -0.08, 0.5, 0.44, WOLF_FUR);
    f.tri(p, [(-0.3, 0.08), (0.3, 0.08), (0.0, 0.62)], WOLF_FUR);
    f.tri(p, [(-0.16, 0.16), (0.16, 0.16), (0.0, 0.56)], WOLF_LIGHT);
    f.circle(p, 0.0, 0.56, 0.08, INK);
    eyes(p, &f, pose, t, 0.22, -0.2, 0.09, WOLF_EYE);
    let stroke = f.stroke();
    line(p, vec![f.pt(-0.32, -0.4), f.pt(-0.1, -0.32)], stroke);
    line(p, vec![f.pt(0.32, -0.4), f.pt(0.1, -0.32)], stroke);
    if pose == Pose::Done {
        smile(p, &f);
    }
    if pose == Pose::Sleeping {
        zzz(p, &f, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, LayerId, RawInput, Shape as S};

    const SIZES: [f32; 3] = [24.0, 40.0, 56.0];
    const POSES: [Pose; 4] = [Pose::Idle, Pose::Working, Pose::Sleeping, Pose::Done];

    /// Render through a headless egui Context and return the emitted shapes.
    fn render(mut f: impl FnMut(&Painter)) -> Vec<S> {
        let ctx = Context::default();
        let out = ctx.run(RawInput::default(), |ctx| {
            let p = ctx.layer_painter(LayerId::background());
            f(&p);
        });
        out.shapes.into_iter().map(|cs| cs.shape).collect()
    }

    fn fills(shapes: &[S]) -> Vec<Color32> {
        shapes
            .iter()
            .filter_map(|s| match s {
                S::Circle(c) => Some(c.fill),
                S::Ellipse(e) => Some(e.fill),
                S::Path(p) => Some(p.fill),
                _ => None,
            })
            .collect()
    }

    fn bear(s: f32, pose: Pose, t: f64) -> Vec<S> {
        render(|p| draw_bear(p, pos2(100.0, 100.0), s, pose, t))
    }
    fn wolf(s: f32, pose: Pose, t: f64) -> Vec<S> {
        render(|p| draw_wolf(p, pos2(100.0, 100.0), s, pose, t))
    }

    #[test]
    fn bear_and_wolf_draw_all_poses_at_all_sizes_within_bounds() {
        let c = pos2(100.0, 100.0);
        for &s in &SIZES {
            for &pose in &POSES {
                for shapes in [bear(s, pose, 0.5), wolf(s, pose, 0.5)] {
                    assert!(shapes.len() >= 6, "too few shapes at s={s} pose={pose:?}");
                    for sh in &shapes {
                        let b = sh.visual_bounding_rect();
                        assert!(
                            b.min.x >= c.x - s && b.max.x <= c.x + s && b.min.y >= c.y - s && b.max.y <= c.y + s,
                            "shape out of bounds at s={s} pose={pose:?}: {b:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn bear_uses_bear_palette_and_wolf_uses_wolf_palette() {
        for &s in &SIZES {
            for &pose in &POSES {
                let b = fills(&bear(s, pose, 0.5));
                let w = fills(&wolf(s, pose, 0.5));
                assert!(b.contains(&BEAR_FUR) && !b.contains(&WOLF_FUR), "bear palette s={s} {pose:?}");
                assert!(w.contains(&WOLF_FUR) && !w.contains(&BEAR_FUR), "wolf palette s={s} {pose:?}");
            }
        }
    }

    #[test]
    fn poses_differ_from_each_other() {
        for &s in &SIZES {
            for draw in [bear as fn(f32, Pose, f64) -> Vec<S>, wolf] {
                let outs: Vec<String> = POSES.iter().map(|&p| format!("{:?}", draw(s, p, 0.5))).collect();
                for i in 0..outs.len() {
                    for j in i + 1..outs.len() {
                        assert_ne!(outs[i], outs[j], "poses {:?} and {:?} identical at s={s}", POSES[i], POSES[j]);
                    }
                }
            }
        }
    }

    #[test]
    fn idle_animates_with_t() {
        for &s in &SIZES {
            assert_ne!(format!("{:?}", bear(s, Pose::Idle, 0.0)), format!("{:?}", bear(s, Pose::Idle, 1.0)));
            assert_ne!(format!("{:?}", wolf(s, Pose::Working, 0.0)), format!("{:?}", wolf(s, Pose::Working, 0.1)));
        }
    }

    #[test]
    fn sleeping_adds_zzz_strokes() {
        for &s in &SIZES {
            assert!(bear(s, Pose::Sleeping, 0.5).len() > bear(s, Pose::Idle, 0.5).len());
            assert!(wolf(s, Pose::Sleeping, 0.5).len() > wolf(s, Pose::Idle, 0.5).len());
        }
    }

    #[test]
    fn zero_or_tiny_size_does_not_panic() {
        for &s in &[0.0f32, 1.0] {
            for &pose in &POSES {
                let _ = bear(s, pose, 0.0);
                let _ = wolf(s, pose, 0.0);
            }
        }
    }
}
