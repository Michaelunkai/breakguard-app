#![allow(dead_code)]
//! Follow-tail state for the lane log page. std-only.

const BOTTOM_EPS: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowState {
    pub follow: bool,
}

impl Default for FollowState {
    fn default() -> Self {
        FollowState { follow: true }
    }
}

impl FollowState {
    /// NaN in either argument: comparisons are false, state unchanged.
    pub fn on_scroll(&mut self, offset: f32, max_offset: f32, user_moved: bool) {
        let bottom = max_offset - BOTTOM_EPS;
        if offset >= bottom {
            self.follow = true;
        } else if user_moved && offset < bottom {
            self.follow = false;
        }
    }

    /// Resume following. Returns the bottom flag as 1.0.
    pub fn jump_to_live(&mut self) -> f32 {
        self.follow = true;
        1.0
    }

    pub fn show_jump_button(&self) -> bool {
        !self.follow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_follows() {
        let s = FollowState::default();
        assert!(s.follow);
        assert!(!s.show_jump_button());
    }

    #[test]
    fn scroll_up_stops_follow() {
        let mut s = FollowState::default();
        s.on_scroll(100.0, 500.0, true);
        assert!(!s.follow);
    }

    #[test]
    fn return_to_bottom_resumes() {
        let mut s = FollowState::default();
        s.on_scroll(100.0, 500.0, true);
        s.on_scroll(498.0, 500.0, true);
        assert!(s.follow);
        s.on_scroll(100.0, 500.0, true);
        s.on_scroll(500.0, 500.0, false);
        assert!(s.follow);
    }

    #[test]
    fn program_scroll_does_not_stop_follow() {
        let mut s = FollowState::default();
        s.on_scroll(100.0, 500.0, false);
        assert!(s.follow);
    }

    #[test]
    fn threshold_boundary() {
        let mut s = FollowState::default();
        s.on_scroll(497.99, 500.0, true);
        assert!(!s.follow);
        s.on_scroll(498.0, 500.0, true);
        assert!(s.follow);
    }

    #[test]
    fn button_visibility() {
        let mut s = FollowState::default();
        assert!(!s.show_jump_button());
        s.on_scroll(0.0, 500.0, true);
        assert!(s.show_jump_button());
        assert_eq!(s.jump_to_live(), 1.0);
        assert!(s.follow);
        assert!(!s.show_jump_button());
    }

    #[test]
    fn nan_safe_no_change() {
        for start in [true, false] {
            for moved in [true, false] {
                for (o, m) in [
                    (f32::NAN, 500.0),
                    (100.0, f32::NAN),
                    (f32::NAN, f32::NAN),
                ] {
                    let mut s = FollowState { follow: start };
                    s.on_scroll(o, m, moved);
                    assert_eq!(s.follow, start);
                }
            }
        }
    }

    #[test]
    fn infinities_no_panic() {
        let mut s = FollowState::default();
        s.on_scroll(f32::NEG_INFINITY, 500.0, true);
        assert!(!s.follow);
        s.on_scroll(f32::INFINITY, 500.0, false);
        assert!(s.follow);
        s.on_scroll(0.0, f32::INFINITY, true);
        assert!(!s.follow);
    }

    #[test]
    fn empty_content_follows() {
        let mut s = FollowState { follow: false };
        s.on_scroll(0.0, 0.0, true);
        assert!(s.follow);
    }

    #[test]
    fn jump_idempotent() {
        let mut s = FollowState::default();
        assert_eq!(s.jump_to_live(), 1.0);
        assert_eq!(s.jump_to_live(), 1.0);
        assert!(s.follow);
    }
}
