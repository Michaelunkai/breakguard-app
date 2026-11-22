//! Icon size grid shared by the nav and compact buttons. Glyphs live in `ui::icons`.
//! API: pub const ICON_SM / ICON_MD / ICON_LG: f32 (14, 17, 20 px boxes)
//! API: pub const NAV_ICON_PX: f32 (17 px, the sidebar icon box)
#![allow(dead_code)] // grid sizes are adopted by Lanes/Widgets/Wiring call sites as they wire up

/// Compact icon-button glyph box (lane actions, toolbar).
pub const ICON_SM: f32 = 14.0;
/// Sidebar nav glyph box.
pub const NAV_ICON_PX: f32 = 17.0;
/// Standard glyph box (cards, stat tiles).
pub const ICON_MD: f32 = NAV_ICON_PX;
/// Large glyph box (empty states, headers).
pub const ICON_LG: f32 = 20.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_grid_is_ordered_and_on_whole_pixels() {
        let grid = [ICON_SM, NAV_ICON_PX, ICON_MD, ICON_LG];
        for pair in grid.windows(2) {
            assert!(pair[0] <= pair[1], "grid not ordered: {grid:?}");
        }
        for px in grid {
            assert_eq!(px.fract(), 0.0, "{px} is not a whole pixel size");
        }
    }
}
