//! Lane mascots drawn with egui painter primitives only (no image assets). 12 distinct creatures,
//! assigned deterministically from the session id; animation follows the lane state.
use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

pub const COUNT: usize = 12;
pub const NAMES: [&str; COUNT] = ["Fox", "Owl", "Cat", "Octopus", "Dragon", "Panda", "Frog", "Penguin", "Bear", "Wolf", "Robot", "Ghost"];

/// Accent palette per creature (lane border, name and cursor).
pub const ACCENTS: [Color32; COUNT] = [
    Color32::from_rgb(251, 146, 60),
    Color32::from_rgb(196, 181, 253),
    Color32::from_rgb(253, 224, 71),
    Color32::from_rgb(244, 114, 182),
    Color32::from_rgb(248, 113, 113),
    Color32::from_rgb(226, 232, 240),
    Color32::from_rgb(134, 239, 172),
    Color32::from_rgb(125, 211, 252),
    Color32::from_rgb(217, 160, 102),
    Color32::from_rgb(165, 180, 252),
    Color32::from_rgb(94, 234, 212),
    Color32::from_rgb(216, 180, 254),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pose {
    Idle,
    Working,
    Sleeping,
    Done,
    Error,
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Stable creature per session id: the id's hash picks a slot; a taken slot moves to the next free
/// one, so up to 12 sessions never share a creature. Ids are processed in sorted order, so the
/// result does not depend on discovery order.
pub fn assign(ids: &[String]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by(|a, b| ids[*a].cmp(&ids[*b]));
    let mut taken = [0usize; COUNT];
    let mut out = vec![0usize; ids.len()];
    for i in order {
        let mut k = (fnv(&ids[i]) % COUNT as u64) as usize;
        // Least-used slot first: no repeats until all 12 are in use.
        let m = *taken.iter().min().unwrap();
        while taken[k] != m {
            k = (k + 1) % COUNT;
        }
        taken[k] += 1;
        out[i] = k;
    }
    out
}

fn poly(p: &Painter, c: Pos2, r: f32, pts: &[(f32, f32)], fill: Color32) {
    p.add(Shape::convex_polygon(pts.iter().map(|(x, y)| c + vec2(*x, *y) * r).collect(), fill, Stroke::NONE));
}
fn dot(p: &Painter, c: Pos2, r: f32, x: f32, y: f32, rr: f32, fill: Color32) {
    p.circle_filled(c + vec2(x, y) * r, rr * r, fill);
}

fn eyes(p: &Painter, c: Pos2, r: f32, y: f32, dx: f32, size: f32, pose: Pose, t: f64) {
    let ink = Color32::from_rgb(15, 23, 42);
    for s in [-1.0f32, 1.0] {
        let e = c + vec2(s * dx, y) * r;
        match pose {
            Pose::Sleeping => {
                p.line_segment([e - vec2(size * r, 0.0), e + vec2(size * r, 0.0)], Stroke::new((0.07 * r).max(1.0), ink));
            }
            Pose::Done => {
                let w = size * r;
                p.line_segment([e + vec2(-w, 0.3 * w), e + vec2(0.0, -0.5 * w)], Stroke::new((0.07 * r).max(1.0), ink));
                p.line_segment([e + vec2(0.0, -0.5 * w), e + vec2(w, 0.3 * w)], Stroke::new((0.07 * r).max(1.0), ink));
            }
            _ => {
                let blink = pose == Pose::Idle && (t % 4.0) > 3.85;
                let h = if blink { 0.15 } else { 1.0 };
                p.add(Shape::ellipse_filled(e, vec2(size, size * h) * r, Color32::WHITE));
                if !blink {
                    let look = if pose == Pose::Working { ((t * 3.0).sin() as f32) * 0.3 } else { 0.0 };
                    p.circle_filled(e + vec2(look * size * r, 0.0), size * 0.55 * r, ink);
                }
            }
        }
    }
}

/// Draw creature `kind` in a square of side `size` centred at `c`. `t` is seconds (animation clock).
pub fn draw(p: &Painter, kind: usize, c: Pos2, size: f32, pose: Pose, t: f64) {
    // Error reuses the Idle body, then adds a pulsing red ring and an alert mark on top.
    if pose == Pose::Error {
        draw(p, kind, c, size, Pose::Idle, t);
        let red = Color32::from_rgb(239, 68, 68);
        let pulse = 0.5 + 0.5 * ((t * 5.0).sin() as f32);
        p.circle_stroke(c, size * (0.62 + 0.04 * pulse), Stroke::new((0.07 * size).max(1.0), red));
        let tip = c + vec2(0.82, -0.82) * size * 0.5;
        p.circle_filled(tip, size * 0.13, red);
        p.text(tip, Align2::CENTER_CENTER, "!", FontId::proportional((size * 0.2).max(9.0)), Color32::WHITE);
        return;
    }
    let c0 = c;
    let mut c = c;
    let mut r = size * 0.5;
    match pose {
        Pose::Idle | Pose::Error => r *= 1.0 + 0.03 * (t * 1.6).sin() as f32,
        Pose::Working => c.y += ((t * 9.0).sin() as f32) * size * 0.025,
        Pose::Sleeping => c.y += size * 0.05,
        Pose::Done => c.y -= (((t * 4.0).sin().abs()) as f32) * size * 0.03,
    }
    let kind = kind % COUNT;
    {
        use crate::ui::parts::{creat_bear_wolf as bw, creat_dragon_panda as dp, creat_frog_penguin as fp, creat_robot_ghost as rg};
        match kind {
            4 => return dp::draw_dragon(p, c0, size, pose, t),
            5 => return dp::draw_panda(p, c0, size, pose, t),
            6 => return fp::draw_frog(p, c0, size, pose, t),
            7 => return fp::draw_penguin(p, c0, size, pose, t),
            8 => return bw::draw_bear(p, c0, size, pose, t),
            9 => return bw::draw_wolf(p, c0, size, pose, t),
            10 => return rg::draw_robot(p, c0, size, pose, t),
            11 => return rg::draw_ghost(p, c0, size, pose, t),
            _ => {}
        }
    }
    let ink = Color32::from_rgb(15, 23, 42);
    let white = Color32::from_rgb(248, 250, 252);
    match kind {
        0 => {
            // fox
            let o = Color32::from_rgb(234, 108, 30);
            poly(p, c, r, &[(-0.85, -0.2), (-0.7, -1.0), (-0.15, -0.55)], o);
            poly(p, c, r, &[(0.85, -0.2), (0.7, -1.0), (0.15, -0.55)], o);
            poly(p, c, r, &[(-0.9, -0.15), (0.9, -0.15), (0.0, 0.95)], o);
            poly(p, c, r, &[(-0.5, 0.25), (0.5, 0.25), (0.0, 0.95)], white);
            dot(p, c, r, 0.0, 0.8, 0.1, ink);
            eyes(p, c, r, -0.05, 0.38, 0.13, pose, t);
        }
        1 => {
            // owl
            let b = Color32::from_rgb(146, 108, 70);
            dot(p, c, r, 0.0, 0.05, 0.92, b);
            poly(p, c, r, &[(-0.8, -0.55), (-0.55, -1.0), (-0.25, -0.7)], b);
            poly(p, c, r, &[(0.8, -0.55), (0.55, -1.0), (0.25, -0.7)], b);
            dot(p, c, r, -0.36, -0.1, 0.33, Color32::from_rgb(254, 243, 199));
            dot(p, c, r, 0.36, -0.1, 0.33, Color32::from_rgb(254, 243, 199));
            eyes(p, c, r, -0.1, 0.36, 0.2, pose, t);
            poly(p, c, r, &[(-0.12, 0.15), (0.12, 0.15), (0.0, 0.4)], Color32::from_rgb(251, 191, 36));
        }
        2 => {
            // cat
            let g = Color32::from_rgb(120, 130, 150);
            poly(p, c, r, &[(-0.85, 0.0), (-0.8, -0.95), (-0.2, -0.5)], g);
            poly(p, c, r, &[(0.85, 0.0), (0.8, -0.95), (0.2, -0.5)], g);
            dot(p, c, r, 0.0, 0.15, 0.85, g);
            eyes(p, c, r, 0.0, 0.38, 0.14, pose, t);
            poly(p, c, r, &[(-0.07, 0.25), (0.07, 0.25), (0.0, 0.35)], Color32::from_rgb(244, 114, 182));
            for s in [-1.0f32, 1.0] {
                for dy in [0.0f32, 0.12] {
                    p.line_segment([c + vec2(s * 0.25, 0.4 + dy) * r, c + vec2(s * 0.9, 0.35 + dy * 2.0) * r], Stroke::new(1.0f32, white));
                }
            }
        }
        3 => {
            // octopus
            let m = Color32::from_rgb(219, 90, 160);
            dot(p, c, r, 0.0, -0.2, 0.75, m);
            for i in 0..4 {
                let x = -0.6 + i as f32 * 0.4;
                let sway = ((t * 3.0 + i as f64).sin() as f32) * if pose == Pose::Working { 0.12 } else { 0.04 };
                p.line_segment([c + vec2(x, 0.3) * r, c + vec2(x + sway, 0.95) * r], Stroke::new(0.2 * r, m));
            }
            eyes(p, c, r, -0.2, 0.3, 0.16, pose, t);
        }
        4 => {
            // dragon
            let gr = Color32::from_rgb(220, 70, 70);
            poly(p, c, r, &[(-0.5, -0.6), (-0.4, -1.0), (-0.15, -0.6)], Color32::from_rgb(251, 191, 36));
            poly(p, c, r, &[(0.5, -0.6), (0.4, -1.0), (0.15, -0.6)], Color32::from_rgb(251, 191, 36));
            dot(p, c, r, 0.0, -0.05, 0.8, gr);
            p.add(Shape::ellipse_filled(c + vec2(0.0, 0.5) * r, vec2(0.5, 0.3) * r, Color32::from_rgb(252, 165, 165)));
            dot(p, c, r, -0.15, 0.5, 0.05, ink);
            dot(p, c, r, 0.15, 0.5, 0.05, ink);
            eyes(p, c, r, -0.2, 0.35, 0.13, pose, t);
        }
        5 => {
            // panda
            dot(p, c, r, -0.7, -0.7, 0.3, ink);
            dot(p, c, r, 0.7, -0.7, 0.3, ink);
            dot(p, c, r, 0.0, 0.0, 0.9, white);
            p.add(Shape::ellipse_filled(c + vec2(-0.36, -0.05) * r, vec2(0.2, 0.28) * r, ink));
            p.add(Shape::ellipse_filled(c + vec2(0.36, -0.05) * r, vec2(0.2, 0.28) * r, ink));
            eyes(p, c, r, -0.05, 0.36, 0.1, pose, t);
            dot(p, c, r, 0.0, 0.3, 0.1, ink);
        }
        6 => {
            // frog
            let gr = Color32::from_rgb(74, 190, 100);
            p.add(Shape::ellipse_filled(c + vec2(0.0, 0.2) * r, vec2(0.95, 0.7) * r, gr));
            dot(p, c, r, -0.45, -0.5, 0.33, gr);
            dot(p, c, r, 0.45, -0.5, 0.33, gr);
            eyes(p, c, r, -0.5, 0.45, 0.22, pose, t);
            p.line_segment([c + vec2(-0.45, 0.45) * r, c + vec2(0.0, 0.62) * r], Stroke::new((0.06 * r).max(1.0), ink));
            p.line_segment([c + vec2(0.0, 0.62) * r, c + vec2(0.45, 0.45) * r], Stroke::new((0.06 * r).max(1.0), ink));
        }
        7 => {
            // penguin
            p.add(Shape::ellipse_filled(c, vec2(0.8, 0.95) * r, Color32::from_rgb(30, 41, 59)));
            p.add(Shape::ellipse_filled(c + vec2(0.0, 0.25) * r, vec2(0.5, 0.6) * r, white));
            eyes(p, c, r, -0.4, 0.28, 0.13, pose, t);
            poly(p, c, r, &[(-0.14, -0.18), (0.14, -0.18), (0.0, 0.02)], Color32::from_rgb(251, 146, 60));
        }
        8 => {
            // bear
            let br = Color32::from_rgb(140, 90, 55);
            dot(p, c, r, -0.7, -0.65, 0.28, br);
            dot(p, c, r, 0.7, -0.65, 0.28, br);
            dot(p, c, r, 0.0, 0.0, 0.88, br);
            p.add(Shape::ellipse_filled(c + vec2(0.0, 0.3) * r, vec2(0.38, 0.3) * r, Color32::from_rgb(222, 184, 135)));
            dot(p, c, r, 0.0, 0.22, 0.1, ink);
            eyes(p, c, r, -0.25, 0.36, 0.1, pose, t);
        }
        9 => {
            // wolf
            let g = Color32::from_rgb(100, 116, 139);
            poly(p, c, r, &[(-0.9, 0.0), (-0.75, -1.0), (-0.1, -0.5)], g);
            poly(p, c, r, &[(0.9, 0.0), (0.75, -1.0), (0.1, -0.5)], g);
            poly(p, c, r, &[(-0.9, -0.05), (0.9, -0.05), (0.35, 0.95), (-0.35, 0.95)], g);
            poly(p, c, r, &[(-0.35, 0.3), (0.35, 0.3), (0.0, 0.95)], Color32::from_rgb(203, 213, 225));
            dot(p, c, r, 0.0, 0.85, 0.09, ink);
            eyes(p, c, r, -0.05, 0.4, 0.12, pose, t);
        }
        10 => {
            // robot
            let m = Color32::from_rgb(148, 163, 184);
            p.line_segment([c + vec2(0.0, -0.95) * r, c + vec2(0.0, -0.6) * r], Stroke::new((0.06 * r).max(1.0), m));
            let on = if pose == Pose::Working { (t * 6.0).sin() > 0.0 } else { pose != Pose::Sleeping };
            dot(p, c, r, 0.0, -0.98, 0.1, if on { Color32::from_rgb(74, 222, 128) } else { Color32::from_rgb(100, 116, 139) });
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 0.05) * r, vec2(1.6, 1.3) * r), 0.2 * r, m);
            eyes(p, c, r, -0.1, 0.4, 0.2, pose, t);
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 0.5) * r, vec2(0.8, 0.14) * r), 0.05 * r, ink);
        }
        _ => {
            // ghost
            let gh = Color32::from_rgb(226, 232, 255);
            dot(p, c, r, 0.0, -0.2, 0.75, gh);
            p.rect_filled(Rect::from_min_max(c + vec2(-0.75, -0.2) * r, c + vec2(0.75, 0.65) * r), 0.0, gh);
            for i in 0..3 {
                dot(p, c, r, -0.5 + i as f32 * 0.5, 0.65, 0.25, gh);
            }
            eyes(p, c, r, -0.25, 0.3, 0.13, pose, t);
            dot(p, c, r, 0.0, 0.1, 0.08, ink);
        }
    }
    if pose == Pose::Sleeping {
        let f = FontId::proportional((size * 0.28).max(9.0));
        let k = (t * 0.8) as f32 % 1.0;
        p.text(pos2(c.x + size * 0.45, c.y - size * (0.35 + 0.2 * k)), Align2::CENTER_CENTER, "z", f.clone(), Color32::from_white_alpha((255.0 * (1.0 - k)) as u8));
        p.text(pos2(c.x + size * 0.62, c.y - size * 0.55), Align2::CENTER_CENTER, "Z", f, Color32::from_white_alpha(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_to_twelve_sessions_never_share_a_creature() {
        for n in 1..=COUNT {
            let ids: Vec<String> = (0..n).map(|i| format!("local_{:x}-{}", i * 7919, n)).collect();
            let a = assign(&ids);
            let mut s = a.clone();
            s.sort();
            s.dedup();
            assert_eq!(s.len(), n, "n={n} got {a:?}");
        }
    }

    #[test]
    fn assignment_is_stable_regardless_of_order() {
        let ids: Vec<String> = ["a1", "b2", "c3", "d4"].iter().map(|s| s.to_string()).collect();
        let a = assign(&ids);
        let rev: Vec<String> = ids.iter().rev().cloned().collect();
        let b = assign(&rev);
        for (i, id) in ids.iter().enumerate() {
            let j = rev.iter().position(|x| x == id).unwrap();
            assert_eq!(a[i], b[j]);
        }
    }

    #[test]
    fn twenty_sessions_give_at_least_twelve_distinct_creatures_stably() {
        let ids: Vec<String> = (0..20).map(|i| format!("sess-{i:03}-{:x}", i * 4621)).collect();
        let a = assign(&ids);
        assert_eq!(a, assign(&ids), "mapping must be stable across calls");
        let mut s = a.clone();
        s.sort();
        s.dedup();
        assert!(s.len() >= 12, "only {} distinct creatures: {a:?}", s.len());
    }

    #[test]
    fn error_pose_draws_without_panic_for_every_kind() {
        let ctx = eframe::egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let p = ctx.layer_painter(eframe::egui::LayerId::background());
            for k in 0..COUNT {
                draw(&p, k, pos2(50.0, 50.0), 40.0, Pose::Error, 1.0);
                draw(&p, k, pos2(50.0, 50.0), 40.0, Pose::Idle, 1.0);
            }
        });
    }

    #[test]
    fn more_than_twelve_still_assigns_valid_kinds() {
        let ids: Vec<String> = (0..30).map(|i| format!("s{i}")).collect();
        assert!(assign(&ids).iter().all(|k| *k < COUNT));
    }
}
