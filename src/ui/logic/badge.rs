#![allow(dead_code)]
use std::collections::HashMap;

#[derive(Default, Debug, Clone)]
pub struct Badges {
    seen: HashMap<String, u64>,
}

impl Badges {
    /// Errors since last mark_seen. Never-seen id counts all. Backwards counter -> 0.
    pub fn unseen(&self, id: &str, total_errors: u64) -> u64 {
        total_errors.saturating_sub(self.seen.get(id).copied().unwrap_or(0))
    }

    pub fn mark_seen(&mut self, id: &str, total_errors: u64) {
        self.seen.insert(id.to_string(), total_errors);
    }

    pub fn nav_total(&self, errors: &[(String, u64)]) -> u64 {
        errors
            .iter()
            .fold(0u64, |acc, (id, n)| acc.saturating_add(self.unseen(id, *n)))
    }
}

pub fn label(n: u64) -> String {
    match n {
        0 => String::new(),
        1..=9 => n.to_string(),
        _ => "9+".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, n: u64) -> (String, u64) {
        (id.to_string(), n)
    }

    #[test]
    fn never_seen_counts_all() {
        assert_eq!(Badges::default().unseen("a", 7), 7);
    }

    #[test]
    fn new_errors_arrive() {
        let mut b = Badges::default();
        b.mark_seen("a", 5);
        assert_eq!(b.unseen("a", 5), 0);
        assert_eq!(b.unseen("a", 9), 4);
    }

    #[test]
    fn mark_seen_clears() {
        let mut b = Badges::default();
        assert_eq!(b.unseen("a", 3), 3);
        b.mark_seen("a", 3);
        assert_eq!(b.unseen("a", 3), 0);
    }

    #[test]
    fn ring_reset_no_underflow_then_recovers() {
        let mut b = Badges::default();
        b.mark_seen("a", 50);
        assert_eq!(b.unseen("a", 10), 0);
        b.mark_seen("a", 10);
        assert_eq!(b.unseen("a", 13), 3);
    }

    #[test]
    fn ids_independent() {
        let mut b = Badges::default();
        b.mark_seen("a", 5);
        assert_eq!(b.unseen("b", 5), 5);
    }

    #[test]
    fn nav_total_sums() {
        let mut b = Badges::default();
        b.mark_seen("a", 2);
        let v = vec![e("a", 5), e("b", 4), e("c", 0)];
        assert_eq!(b.nav_total(&v), 7);
        assert_eq!(b.nav_total(&[]), 0);
    }

    #[test]
    fn nav_total_saturates() {
        let b = Badges::default();
        assert_eq!(b.nav_total(&[e("a", u64::MAX), e("b", 5)]), u64::MAX);
    }

    #[test]
    fn nav_total_duplicate_ids_and_backwards() {
        let mut b = Badges::default();
        b.mark_seen("a", 10);
        assert_eq!(b.nav_total(&[e("a", 3), e("a", 12)]), 2);
    }

    #[test]
    fn label_cases() {
        assert_eq!(label(0), "");
        assert_eq!(label(1), "1");
        assert_eq!(label(9), "9");
        assert_eq!(label(10), "9+");
        assert_eq!(label(99), "9+");
        assert_eq!(label(u64::MAX), "9+");
    }

    #[test]
    fn adversarial_ids_and_extremes() {
        let mut b = Badges::default();
        b.mark_seen("", u64::MAX);
        b.mark_seen("שלום\0🦀", 1);
        assert_eq!(b.unseen("", 0), 0);
        assert_eq!(b.unseen("", u64::MAX), 0);
        assert_eq!(b.unseen("שלום\0🦀", 3), 2);
        assert_eq!(b.unseen("A", 1), 1);
        assert_eq!(b.unseen("a", 1), 1);
    }
}
