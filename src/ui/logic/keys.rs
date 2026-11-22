#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    J,
    K,
    F,
    Space,
    Escape,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    None,
    FocusFilter,
    ToggleFreeze(usize),
    Blur,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nav {
    pub selected: usize,
    pub filter_focus: bool,
}

impl Nav {
    pub fn handle(&mut self, key: Key, lane_count: usize, typing: bool) -> Out {
        if typing && key != Key::Escape {
            return Out::None;
        }
        // lanes may have shrunk since last call
        if lane_count > 0 && self.selected >= lane_count {
            self.selected = lane_count - 1;
        }
        match key {
            Key::J => {
                if lane_count > 0 {
                    self.selected = if self.selected + 1 < lane_count {
                        self.selected + 1
                    } else {
                        0
                    };
                }
                Out::None
            }
            Key::K => {
                if lane_count > 0 {
                    self.selected = if self.selected == 0 {
                        lane_count - 1
                    } else {
                        self.selected - 1
                    };
                }
                Out::None
            }
            Key::F => {
                self.filter_focus = true;
                Out::FocusFilter
            }
            Key::Space => {
                if lane_count == 0 {
                    Out::None
                } else {
                    Out::ToggleFreeze(self.selected)
                }
            }
            Key::Escape => {
                self.filter_focus = false;
                Out::Blur
            }
            Key::Other => Out::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_sequence() {
        let mut n = Nav::default();
        assert_eq!(n.handle(Key::J, 3, false), Out::None);
        assert_eq!(n.handle(Key::J, 3, false), Out::None);
        assert_eq!(n.selected, 2);
        assert_eq!(n.handle(Key::J, 3, false), Out::None);
        assert_eq!(n.selected, 0, "wrap at end");
        assert_eq!(n.handle(Key::Space, 3, false), Out::ToggleFreeze(0));
        assert_eq!(n.handle(Key::K, 3, false), Out::None);
        assert_eq!(n.selected, 2, "wrap at start");
        assert_eq!(n.handle(Key::F, 3, false), Out::FocusFilter);
        assert!(n.filter_focus);
        assert_eq!(n.handle(Key::Escape, 3, false), Out::Blur);
        assert!(!n.filter_focus);
        assert_eq!(n.handle(Key::Other, 3, false), Out::None);
        assert_eq!(n.selected, 2);
    }

    #[test]
    fn k_wraps_at_zero() {
        let mut n = Nav::default();
        assert_eq!(n.handle(Key::K, 3, false), Out::None);
        assert_eq!(n.selected, 2);
        n.handle(Key::K, 3, false);
        assert_eq!(n.selected, 1);
        n.handle(Key::K, 3, false);
        assert_eq!(n.selected, 0);
    }

    #[test]
    fn j_wraps_full_cycle() {
        let mut n = Nav::default();
        for expected in [1, 2, 0, 1] {
            n.handle(Key::J, 3, false);
            assert_eq!(n.selected, expected);
        }
    }

    #[test]
    fn empty_lanes() {
        let mut n = Nav::default();
        assert_eq!(n.handle(Key::J, 0, false), Out::None);
        assert_eq!(n.handle(Key::K, 0, false), Out::None);
        assert_eq!(n.handle(Key::Space, 0, false), Out::None);
        assert_eq!(n.selected, 0);
        assert_eq!(n.handle(Key::F, 0, false), Out::FocusFilter);
        assert_eq!(n.handle(Key::Escape, 0, false), Out::Blur);
    }

    #[test]
    fn single_lane() {
        let mut n = Nav::default();
        n.handle(Key::J, 1, false);
        assert_eq!(n.selected, 0);
        n.handle(Key::K, 1, false);
        assert_eq!(n.selected, 0);
        assert_eq!(n.handle(Key::Space, 1, false), Out::ToggleFreeze(0));
    }

    #[test]
    fn typing_guard() {
        let mut n = Nav::default();
        for k in [Key::J, Key::K, Key::F, Key::Space, Key::Other] {
            assert_eq!(n.handle(k, 5, true), Out::None);
        }
        assert_eq!(n, Nav::default(), "no state change while typing");
        n.filter_focus = true;
        assert_eq!(n.handle(Key::Escape, 5, true), Out::Blur);
        assert!(!n.filter_focus);
    }

    #[test]
    fn lanes_shrink_clamps_selection() {
        let mut n = Nav { selected: 9, filter_focus: false };
        assert_eq!(n.handle(Key::Space, 3, false), Out::ToggleFreeze(2));
        assert_eq!(n.selected, 2);
        let mut m = Nav { selected: 9, filter_focus: false };
        m.handle(Key::J, 4, false);
        assert_eq!(m.selected, 0, "clamped to 3, then wraps");
    }

    #[test]
    fn huge_selected_and_lane_count() {
        let mut n = Nav { selected: usize::MAX, filter_focus: false };
        assert_eq!(n.handle(Key::J, usize::MAX, false), Out::None);
        assert_eq!(n.selected, 0);
        let mut z = Nav { selected: usize::MAX, filter_focus: false };
        assert_eq!(z.handle(Key::J, 0, false), Out::None);
        assert_eq!(z.handle(Key::Space, 0, false), Out::None);
        assert_eq!(z.handle(Key::K, 0, false), Out::None);
        assert_eq!(z.selected, usize::MAX);
    }

    #[test]
    fn repeated_f_idempotent() {
        let mut n = Nav::default();
        assert_eq!(n.handle(Key::F, 2, false), Out::FocusFilter);
        assert_eq!(n.handle(Key::F, 2, false), Out::FocusFilter);
        assert!(n.filter_focus);
    }
}
