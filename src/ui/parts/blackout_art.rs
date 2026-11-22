//! Pure model for the blackout screen art: clock text, message rotation,
//! creature pose choice and dim alpha curve. No egui types here.
//!
//! Invariant: Esc always exits the blackout, in every state (see `esc_exits`).

#![allow(dead_code)]

/// Blackout screen state. Every variant must allow Esc to exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlackoutState {
    Idle,
    Active,
    Fading,
    Ending,
}

/// Creature pose shown on the blackout screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pose {
    Still,
    Stalk,
    Crouch,
    Lunge,
}

/// Messages rotated on the blackout screen.
pub const MESSAGES: &[&str] = &[
    "Rest your eyes.",
    "Look at something far away.",
    "Breathe slowly.",
    "Stretch your neck.",
];

/// Seconds per message before rotating to the next one.
pub const MESSAGE_PERIOD_SECS: u64 = 15;

/// Soft clock text `mm:ss` for an elapsed number of seconds.
/// Minutes wrap at 100 so the text width stays fixed.
pub fn clock_text(secs: u64) -> String {
    format!("{:02}:{:02}", (secs / 60) % 100, secs % 60)
}

/// Message shown at `secs` elapsed, rotating every `MESSAGE_PERIOD_SECS`.
pub fn message_at(secs: u64) -> &'static str {
    let idx = (secs / MESSAGE_PERIOD_SECS) as usize % MESSAGES.len();
    MESSAGES[idx]
}

/// Creature pose at `secs` elapsed. Cycles through a fixed 4-step pattern.
pub fn pose_for(secs: u64) -> Pose {
    match (secs / 10) % 4 {
        0 => Pose::Still,
        1 => Pose::Stalk,
        2 => Pose::Crouch,
        _ => Pose::Lunge,
    }
}

/// Small text glyph for a creature pose, drawn under the clock.
pub fn pose_glyph(p: Pose) -> &'static str {
    match p {
        Pose::Still => "(-_-)",
        Pose::Stalk => "(._.)",
        Pose::Crouch => "(v_v)",
        Pose::Lunge => "(O_O)!",
    }
}

/// Dim alpha (0 = transparent, 255 = fully dim) after `elapsed_ms` of a
/// `fade_ms` linear fade-in. Clamped to 0..=255; zero fade is instant.
pub fn dim_alpha(elapsed_ms: u32, fade_ms: u32) -> u8 {
    if fade_ms == 0 || elapsed_ms >= fade_ms {
        return 255;
    }
    ((elapsed_ms as u64 * 255) / fade_ms as u64) as u8
}

/// Esc exits the blackout in every state. Must stay `true` for all states.
pub fn esc_exits(_state: BlackoutState) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_STATES: [BlackoutState; 4] = [
        BlackoutState::Idle,
        BlackoutState::Active,
        BlackoutState::Fading,
        BlackoutState::Ending,
    ];

    #[test]
    fn esc_exits_in_every_state() {
        for s in ALL_STATES {
            assert!(esc_exits(s), "Esc must exit in {:?}", s);
        }
    }

    #[test]
    fn clock_text_formats_mm_ss() {
        assert_eq!(clock_text(0), "00:00");
        assert_eq!(clock_text(9), "00:09");
        assert_eq!(clock_text(65), "01:05");
        assert_eq!(clock_text(3599), "59:59");
        assert_eq!(clock_text(3600), "60:00");
        assert_eq!(clock_text(6000), "00:00");
    }

    #[test]
    fn message_rotates_every_period() {
        assert_eq!(message_at(0), MESSAGES[0]);
        assert_eq!(message_at(MESSAGE_PERIOD_SECS - 1), MESSAGES[0]);
        assert_eq!(message_at(MESSAGE_PERIOD_SECS), MESSAGES[1]);
        let wrap = MESSAGE_PERIOD_SECS * MESSAGES.len() as u64;
        assert_eq!(message_at(wrap), MESSAGES[0]);
    }

    #[test]
    fn pose_cycles_every_10_secs() {
        assert_eq!(pose_for(0), Pose::Still);
        assert_eq!(pose_for(10), Pose::Stalk);
        assert_eq!(pose_for(20), Pose::Crouch);
        assert_eq!(pose_for(30), Pose::Lunge);
        assert_eq!(pose_for(40), Pose::Still);
    }

    #[test]
    fn pose_glyph_distinct_per_pose() {
        let g: Vec<&str> = [Pose::Still, Pose::Stalk, Pose::Crouch, Pose::Lunge].iter().map(|p| pose_glyph(*p)).collect();
        for (i, a) in g.iter().enumerate() {
            assert!(!a.is_empty());
            for b in &g[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn dim_alpha_linear_and_clamped() {
        assert_eq!(dim_alpha(0, 1000), 0);
        assert_eq!(dim_alpha(500, 1000), 127);
        assert_eq!(dim_alpha(1000, 1000), 255);
        assert_eq!(dim_alpha(5000, 1000), 255);
        assert_eq!(dim_alpha(0, 0), 255);
    }
}
