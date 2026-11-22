#![allow(dead_code)]
//! View-only clear: remembers how many lines had been pushed at clear time.
//! The source buffer is never touched.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Cleared {
    pub from: u64,
}

impl Cleared {
    /// Hide everything pushed so far.
    pub fn clear(&mut self, total_pushed: u64) {
        self.from = total_pushed;
    }

    /// Lines pushed after the clear that the ring still holds.
    /// `held` = lines currently in the ring, `dropped` = lines evicted so far.
    pub fn visible_count(&self, total_pushed: u64, held: usize, dropped: u64) -> usize {
        let _ = dropped; // newest `held` lines are the survivors; dropped only ages the oldest
        let after = total_pushed.saturating_sub(self.from);
        after.min(held as u64) as usize
    }

    pub fn reset(&mut self) {
        self.from = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ring of capacity `cap` over a Vec source that is never mutated by the view.
    struct Src {
        v: Vec<String>,
        total: u64,
    }
    impl Src {
        fn new() -> Self {
            Src { v: vec![], total: 0 }
        }
        fn push(&mut self, s: &str) {
            self.v.push(s.to_string());
            self.total += 1;
        }
        fn held(&self, cap: usize) -> usize {
            self.v.len().min(cap)
        }
        fn dropped(&self, cap: usize) -> u64 {
            (self.v.len().saturating_sub(cap)) as u64
        }
        fn view(&self, c: &Cleared, cap: usize) -> Vec<String> {
            let n = c.visible_count(self.total, self.held(cap), self.dropped(cap));
            self.v[self.v.len() - n..].to_vec()
        }
    }

    #[test]
    fn default_zero_shows_all() {
        let c = Cleared::default();
        assert_eq!(c.from, 0);
        assert_eq!(c.visible_count(5, 5, 0), 5);
        assert_eq!(c.visible_count(0, 0, 0), 0);
    }

    #[test]
    fn clear_hides_existing() {
        let mut c = Cleared::default();
        c.clear(5);
        assert_eq!(c.visible_count(5, 5, 0), 0);
        assert_eq!(c.visible_count(7, 7, 0), 2);
    }

    #[test]
    fn source_untouched_view_only_new() {
        let mut s = Src::new();
        for i in 0..4 {
            s.push(&format!("old{i}"));
        }
        let mut c = Cleared::default();
        c.clear(s.total);
        assert_eq!(s.v.len(), 4);
        assert!(s.view(&c, 100).is_empty());
        s.push("new0");
        s.push("new1");
        assert_eq!(s.v.len(), 6);
        assert_eq!(s.view(&c, 100), vec!["new0", "new1"]);
    }

    #[test]
    fn after_ring_drops() {
        let cap = 5;
        let mut s = Src::new();
        for i in 0..4 {
            s.push(&format!("old{i}"));
        }
        let mut c = Cleared::default();
        c.clear(s.total);
        for i in 0..10 {
            s.push(&format!("n{i}"));
        }
        // 10 new but ring holds 5: only 5 visible, none old
        assert_eq!(s.view(&c, cap).len(), 5);
        assert!(s.view(&c, cap).iter().all(|l| l.starts_with('n')));
        assert_eq!(s.v.len(), 14);
    }

    #[test]
    fn ring_drops_past_clear_point() {
        // clear at 3, ring cap 2, 1 new line: 1 visible, never more than held
        let mut c = Cleared::default();
        c.clear(3);
        assert_eq!(c.visible_count(4, 2, 2), 1);
        assert_eq!(c.visible_count(100, 2, 98), 2);
    }

    #[test]
    fn total_below_from_saturates() {
        let mut c = Cleared::default();
        c.clear(50);
        assert_eq!(c.visible_count(10, 10, 0), 0);
        assert_eq!(c.visible_count(0, 0, 0), 0);
    }

    #[test]
    fn reset_restores_all() {
        let mut c = Cleared::default();
        c.clear(9);
        c.reset();
        assert_eq!(c, Cleared::default());
        assert_eq!(c.visible_count(9, 9, 0), 9);
    }

    #[test]
    fn double_clear_and_extremes() {
        let mut c = Cleared::default();
        c.clear(3);
        c.clear(8);
        assert_eq!(c.visible_count(10, 10, 0), 2);
        c.clear(u64::MAX);
        assert_eq!(c.visible_count(u64::MAX, usize::MAX, 0), 0);
        c.reset();
        assert_eq!(c.visible_count(u64::MAX, 7, 0), 7);
        assert_eq!(c.visible_count(u64::MAX, usize::MAX, 0), usize::MAX);
    }

    #[test]
    fn held_zero() {
        let c = Cleared::default();
        assert_eq!(c.visible_count(10, 0, 10), 0);
    }
}
