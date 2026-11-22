//! slice dark_polish (owned by one team agent)
//! API: pub fn surface(layer: u8) -> Color32  (layers 0..=3, strictly lighter, clamped)
//! API: pub fn paint_gradient(p: &egui::Painter, r: egui::Rect, top: Color32, bottom: Color32)
//! API: pub fn panel_gradient(p: &egui::Painter, r: egui::Rect)
#![allow(dead_code)]
use crate::ui::theme::{BG, SURFACE, SURFACE_HI, SURFACE_LO};
use eframe::egui::{
    self,
    epaint::{Mesh, Vertex, WHITE_UV},
    Color32, Rect, Shape,
};

/// Dark layers, darkest to lightest. All theme tokens.
const LAYERS: [Color32; 4] = [BG, SURFACE_LO, SURFACE, SURFACE_HI];

/// Layer colour for elevation `layer` (0 = page background, 3 = highest surface).
/// Values above 3 clamp to 3.
pub fn surface(layer: u8) -> Color32 {
    LAYERS[layer.min(3) as usize]
}

/// Two-triangle vertical gradient mesh: top edge `top`, bottom edge `bottom`.
fn gradient_mesh(r: Rect, top: Color32, bottom: Color32) -> Mesh {
    let mut m = Mesh::default();
    let vert = |pos, color| Vertex { pos, uv: WHITE_UV, color };
    m.vertices.push(vert(r.left_top(), top));
    m.vertices.push(vert(r.right_top(), top));
    m.vertices.push(vert(r.left_bottom(), bottom));
    m.vertices.push(vert(r.right_bottom(), bottom));
    m.indices.extend_from_slice(&[0, 1, 2, 1, 3, 2]);
    m
}

/// Paint a vertical gradient filling `r` (no texture, 2 triangles).
pub fn paint_gradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    p.add(Shape::mesh(gradient_mesh(r, top, bottom)));
}

/// Subtle default panel background: top layer 2 fading to layer 1.
pub fn panel_gradient(p: &egui::Painter, r: Rect) {
    paint_gradient(p, r, surface(2), surface(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::logic::contrast;
    use crate::ui::theme::TEXT;
    use eframe::egui::{pos2, vec2};

    fn rgb(c: Color32) -> (u8, u8, u8) {
        (c.r(), c.g(), c.b())
    }

    #[test]
    fn layers_strictly_lighter() {
        for i in 0..3u8 {
            let lo = contrast::luminance(rgb(surface(i)));
            let hi = contrast::luminance(rgb(surface(i + 1)));
            assert!(lo < hi, "layer {i} ({lo}) not darker than layer {} ({hi})", i + 1);
        }
    }

    #[test]
    fn layer_clamps_above_three() {
        assert_eq!(surface(4), surface(3));
        assert_eq!(surface(255), surface(3));
    }

    #[test]
    fn text_passes_aa_on_every_layer() {
        for i in 0..=3u8 {
            let r = contrast::ratio(rgb(TEXT), rgb(surface(i)));
            assert!(contrast::passes_aa(rgb(TEXT), rgb(surface(i))), "layer {i}: ratio {r}");
        }
    }

    #[test]
    fn gradient_mesh_two_triangles_vertical() {
        let r = Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 50.0));
        let top = Color32::from_rgb(1, 2, 3);
        let bottom = Color32::from_rgb(4, 5, 6);
        let m = gradient_mesh(r, top, bottom);
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.indices.len(), 6);
        assert_eq!(m.indices.len() / 3, 2);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        assert_eq!(m.vertices[0].pos, r.left_top());
        assert_eq!(m.vertices[1].pos, r.right_top());
        assert_eq!(m.vertices[2].pos, r.left_bottom());
        assert_eq!(m.vertices[3].pos, r.right_bottom());
        assert_eq!(m.vertices[0].color, top);
        assert_eq!(m.vertices[1].color, top);
        assert_eq!(m.vertices[2].color, bottom);
        assert_eq!(m.vertices[3].color, bottom);
    }

    #[test]
    fn gradient_mesh_handles_zero_size_rect() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0));
        let m = gradient_mesh(r, surface(0), surface(3));
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.indices.len(), 6);
    }
}
