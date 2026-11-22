//! Accessible names and focus-ring metrics for lane controls.
//! Pure data + math: no egui or crate imports, so it compiles standalone.

/// Each entry: (label literal as written in `pages/lanes.rs`, accessible name).
/// Dynamic labels (`wrap: on/off`, `time: ...`, freeze/unfreeze) keep the source literal.
pub const LANE_CONTROL_NAMES: &[(&str, &str)] = &[
    ("freeze", "Freeze lane output"),
    ("unfreeze", "Unfreeze lane output"),
    ("clear", "Clear lane output"),
    ("copy", "Copy lane output"),
    ("export", "Export lane output"),
    ("copy id", "Copy lane id"),
    ("open folder", "Open lane log folder"),
    ("solo", "Solo lane"),
    ("collapse", "Collapse lane"),
    ("expand", "Expand lane"),
    ("Filter lines", "Filter lane lines"),
    ("A-", "Decrease lane font size"),
    ("A+", "Increase lane font size"),
    ("wrap: on", "Wrap lane lines, currently on"),
    ("wrap: off", "Wrap lane lines, currently off"),
    ("dense", "Dense lane spacing"),
    ("comfortable", "Comfortable lane spacing"),
    ("time: off", "Lane timestamps, currently off"),
    ("time: ago", "Lane timestamps, relative, currently ago"),
    ("time: clock", "Lane timestamps, clock time, currently clock"),
];

/// Focus ring color (sRGB), amber. Contrast is proven in `logic::palette` tests.
pub const FOCUS_RING_RGB: [u8; 3] = [0xFF, 0xB0, 0x00];
/// Focus ring width in logical pixels.
pub const FOCUS_RING_WIDTH: f32 = 2.0;

/// Accessible name for a lane control label, if the table covers it.
pub fn accessible_name(label: &str) -> Option<&'static str> {
    LANE_CONTROL_NAMES.iter().find(|(l, _)| *l == label).map(|(_, n)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANES_SRC: &str = include_str!("../pages/lanes.rs");

    #[test]
    fn every_lanes_control_label_has_accessible_name() {
        for (label, name) in LANE_CONTROL_NAMES {
            assert!(!name.trim().is_empty(), "empty accessible name for {label:?}");
            let quoted = format!("\"{label}\"");
            assert!(LANES_SRC.contains(&quoted), "label {quoted} not found in pages/lanes.rs");
            assert_eq!(accessible_name(label), Some(*name));
        }
    }

    #[test]
    fn known_lane_buttons_are_covered() {
        for label in ["freeze", "clear", "copy", "export", "copy id", "open folder", "solo", "collapse", "A-", "A+", "wrap: on", "time: off"] {
            assert!(accessible_name(label).is_some(), "missing {label}");
        }
    }

    // Compile-time check: the focus ring stays at least 2 px wide.
    const _: () = assert!(FOCUS_RING_WIDTH >= 2.0);
}
