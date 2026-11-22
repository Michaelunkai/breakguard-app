#![allow(dead_code)]
//! Freeze: pure view-freeze logic for a ring-buffered log. std only.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub frozen_len: Option<u64>,
}

impl View {
    pub fn freeze(&mut self, total_pushed: u64) {
        self.frozen_len = Some(total_pushed);
    }

    pub fn unfreeze(&mut self) {
        self.frozen_len = None;
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen_len.is_some()
    }

    /// Frozen: lines that existed at freeze time minus dropped, clamped 0..=held.
    /// Unfrozen: held.
    pub fn visible_count(&self, _total_pushed: u64, held: usize, dropped: u64) -> usize {
        match self.frozen_len {
            None => held,
            Some(f) => {
                let n = f.saturating_sub(dropped);
                if n > held as u64 {
                    held
                } else {
                    n as usize
                }
            }
        }
    }

    /// Lines pushed since freeze; 0 when unfrozen.
    pub fn hidden_new(&self, total_pushed: u64) -> u64 {
        match self.frozen_len {
            None => 0,
            Some(f) => total_pushed.saturating_sub(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_unfrozen() {
        let v = View::default();
        assert!(!v.is_frozen());
        assert_eq!(v.visible_count(10, 7, 3), 7);
        assert_eq!(v.hidden_new(10), 0);
    }

    #[test]
    fn grows_while_frozen_view_stable() {
        let mut v = View::default();
        v.freeze(5);
        assert!(v.is_frozen());
        for total in 5..50u64 {
            assert_eq!(v.visible_count(total, total as usize, 0), 5);
            assert_eq!(v.hidden_new(total), total - 5);
        }
    }

    #[test]
    fn ring_drops_shrink_view() {
        let mut v = View::default();
        v.freeze(100);
        // cap 100: 20 more pushed, 20 dropped, held 100
        assert_eq!(v.visible_count(120, 100, 20), 80);
        assert_eq!(v.hidden_new(120), 20);
    }

    #[test]
    fn all_frozen_lines_dropped_gives_zero() {
        let mut v = View::default();
        v.freeze(10);
        assert_eq!(v.visible_count(500, 10, 490), 0);
        assert_eq!(v.visible_count(500, 10, 10), 0);
        assert_eq!(v.visible_count(500, 10, u64::MAX), 0);
    }

    #[test]
    fn clamped_to_held() {
        let mut v = View::default();
        v.freeze(100);
        assert_eq!(v.visible_count(100, 40, 0), 40);
    }

    #[test]
    fn unfreeze_restores() {
        let mut v = View::default();
        v.freeze(3);
        v.unfreeze();
        assert!(!v.is_frozen());
        assert_eq!(v.visible_count(9, 9, 0), 9);
        assert_eq!(v.hidden_new(9), 0);
    }

    #[test]
    fn refreeze_updates() {
        let mut v = View::default();
        v.freeze(3);
        v.freeze(8);
        assert_eq!(v.frozen_len, Some(8));
        assert_eq!(v.hidden_new(10), 2);
    }

    #[test]
    fn adversarial_extremes() {
        let mut v = View::default();
        v.freeze(u64::MAX);
        assert_eq!(v.visible_count(u64::MAX, usize::MAX, 0), usize::MAX);
        assert_eq!(v.visible_count(u64::MAX, 5, 0), 5);
        assert_eq!(v.hidden_new(u64::MAX), 0);
        // total below frozen (inconsistent input): no underflow
        v.freeze(10);
        assert_eq!(v.hidden_new(3), 0);
        v.freeze(0);
        assert_eq!(v.visible_count(0, 0, 0), 0);
        assert_eq!(v.visible_count(5, 5, 0), 0);
        assert_eq!(v.hidden_new(5), 5);
    }
}
